// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{
    datagram::{UdpDatagram, UdpHeader},
    listener::SharedListener,
    operations::PopFuture,
    socket::Socket,
};
use crate::{
    fail::Fail,
    protocols::{
        arp,
        ethernet2::frame::{EtherType2, Ethernet2Header},
        ipv4,
        ipv4::datagram::{Ipv4Header, Ipv4Protocol2},
    },
    queue::IoQueueDescriptor,
    runtime::Runtime,
    scheduler::SchedulerHandle,
};
use futures::{channel::mpsc, stream::StreamExt};
use std::collections::HashMap;

#[cfg(feature = "profiler")]
use perftools::timer;

//==============================================================================
// Constants & Structures
//==============================================================================

type OutgoingReq<T> = (Option<ipv4::Endpoint>, ipv4::Endpoint, T);
type OutgoingSender<T> = mpsc::UnboundedSender<OutgoingReq<T>>;
type OutgoingReceiver<T> = mpsc::UnboundedReceiver<OutgoingReq<T>>;

///
/// UDP Peer
///
/// # References
///
/// - See https://datatracker.ietf.org/doc/html/rfc768 for details on UDP.
///
pub struct UdpPeer<RT: Runtime> {
    rt: RT,
    arp: arp::Peer<RT>,

    sockets: HashMap<IoQueueDescriptor, Socket>,
    bound: HashMap<ipv4::Endpoint, SharedListener<RT::Buf>>,
    outgoing: OutgoingSender<RT::Buf>,

    #[allow(unused)]
    handle: SchedulerHandle,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [UdpPeer].
impl<RT: Runtime> UdpPeer<RT> {
    /// Creates a Udp peer.
    pub fn new(rt: RT, arp: arp::Peer<RT>) -> Self {
        let (tx, rx) = mpsc::unbounded();
        let future = Self::background(rt.clone(), arp.clone(), rx);
        let handle = rt.spawn(future);
        Self {
            rt,
            arp,
            sockets: HashMap::new(),
            bound: HashMap::new(),
            outgoing: tx,
            handle,
        }
    }

    async fn background(rt: RT, arp: arp::Peer<RT>, mut rx: OutgoingReceiver<RT::Buf>) {
        while let Some((local, remote, buf)) = rx.next().await {
            let r: Result<_, Fail> = try {
                let link_addr = arp.query(remote.addr).await?;
                let datagram = UdpDatagram::new(
                    Ethernet2Header {
                        dst_addr: link_addr,
                        src_addr: rt.local_link_addr(),
                        ether_type: EtherType2::Ipv4,
                    },
                    Ipv4Header::new(rt.local_ipv4_addr(), remote.addr, Ipv4Protocol2::Udp),
                    UdpHeader::new(local.map(|l| l.port), remote.port),
                    buf,
                    rt.udp_options().tx_checksum(),
                );
                rt.transmit(datagram);
            };
            if let Err(e) = r {
                warn!("Failed to send UDP message: {:?}", e);
            }
        }
    }

    /// Opens a UDP socket.
    pub fn do_socket(&mut self, fd: IoQueueDescriptor) {
        #[cfg(feature = "profiler")]
        timer!("udp::socket");

        // Sanity check.
        assert_eq!(
            self.sockets.contains_key(&fd),
            false,
            "file descriptor in use"
        );

        // Open socket.
        let socket = Socket::default();
        self.sockets.insert(fd, socket);
    }

    /// Binds a socket to an endpoint address.
    pub fn bind(&mut self, fd: IoQueueDescriptor, addr: ipv4::Endpoint) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::bind");

        // Endpoint in use.
        if self.bound.contains_key(&addr) {
            return Err(Fail::Malformed {
                details: "Port already listening",
            });
        }

        // Update file descriptor with local endpoint.
        match self.sockets.get_mut(&fd) {
            Some(s) if s.local().is_none() => {
                s.set_local(Some(addr));
            }
            _ => return Err(Fail::BadFileDescriptor {}),
        }

        // Register listener.
        let listener = SharedListener::default();
        if self.bound.insert(addr, listener).is_some() {
            return Err(Fail::AddressInUse {});
        }

        Ok(())
    }

    ///
    /// Dummy accept operation.
    ///
    /// - TODO: we should drop this function because it is meaningless for UDP.
    ///
    pub fn connect(&self, _fd: IoQueueDescriptor, _addr: ipv4::Endpoint) -> Result<(), Fail> {
        Err(Fail::Malformed {
            details: "Operation not supported",
        })
    }

    /// Closes a UDP socket.
    pub fn do_close(&mut self, fd: IoQueueDescriptor) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::close");

        let socket = match self.sockets.remove(&fd) {
            Some(s) => s,
            None => return Err(Fail::BadFileDescriptor {}),
        };

        // Remove endpoint biding.
        if let Some(local) = socket.local() {
            if self.bound.remove(&local).is_none() {
                return Err(Fail::BadFileDescriptor {});
            }
        }

        Ok(())
    }

    /// Consumes the payload from a buffer.
    pub fn receive(&mut self, ipv4_header: &Ipv4Header, buf: RT::Buf) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::receive");

        let (hdr, data) = UdpHeader::parse(ipv4_header, buf, self.rt.udp_options().rx_checksum())?;
        debug!("UDP received {:?}", hdr);
        let local = ipv4::Endpoint::new(ipv4_header.dst_addr, hdr.dest_port());
        let remote = hdr
            .src_port()
            .map(|p| ipv4::Endpoint::new(ipv4_header.src_addr, p));

        // TODO: Send ICMPv4 error in this condition.
        let listener = self.bound.get_mut(&local).ok_or(Fail::Malformed {
            details: "Port not bound",
        })?;

        // Consume data and wakeup receiver.
        listener.push_data(remote, data);
        if let Some(w) = listener.take_waker() {
            w.wake()
        }

        Ok(())
    }

    /// Pushes data to a socket.
    pub fn push(&self, fd: IoQueueDescriptor, buf: RT::Buf) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::push");

        match self.sockets.get(&fd) {
            Some(s) if s.local().is_some() && s.remote().is_some() => {
                self.send_datagram(buf, s.local(), s.remote().unwrap())
            }
            Some(s) if s.local().is_some() => Err(Fail::BadFileDescriptor {}),
            Some(s) if s.remote().is_some() => Err(Fail::BadFileDescriptor {}),
            _ => Err(Fail::Malformed {
                details: "Invalid file descriptor",
            }),
        }
    }

    pub fn pushto(
        &self,
        fd: IoQueueDescriptor,
        buf: RT::Buf,
        to: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::pushto");

        let local = match self.sockets.get(&fd) {
            Some(s) if s.local().is_some() => s.local(),
            _ => return Err(Fail::BadFileDescriptor {}),
        };
        self.send_datagram(buf, local, to)
    }

    /// Sends a UDP packet.
    fn send_datagram(
        &self,
        buf: RT::Buf,
        local: Option<ipv4::Endpoint>,
        remote: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        // First, try to send the packet immediately. If we can't defer the
        // operation to the async path.
        if let Some(link_addr) = self.arp.try_query(remote.addr) {
            let udp_header = UdpHeader::new(local.map(|l| l.port), remote.port);
            debug!("UDP send {:?}", udp_header);
            let datagram = UdpDatagram::new(
                Ethernet2Header {
                    dst_addr: link_addr,
                    src_addr: self.rt.local_link_addr(),
                    ether_type: EtherType2::Ipv4,
                },
                Ipv4Header::new(self.rt.local_ipv4_addr(), remote.addr, Ipv4Protocol2::Udp),
                udp_header,
                buf,
                self.rt.udp_options().tx_checksum(),
            );
            self.rt.transmit(datagram);
        } else {
            self.outgoing.unbounded_send((local, remote, buf)).unwrap();
        }
        Ok(())
    }

    /// Pops data from a socket.
    pub fn pop(&self, fd: IoQueueDescriptor) -> PopFuture<RT> {
        #[cfg(feature = "profiler")]
        timer!("udp::pop");

        let listener = match self.sockets.get(&fd) {
            Some(s) if s.local().is_some() => {
                Ok(self.bound.get(&s.local().unwrap()).unwrap().clone())
            }
            _ => Err(Fail::Malformed {
                details: "Invalid file descriptor",
            }),
        };

        PopFuture::new(fd, listener)
    }
}
