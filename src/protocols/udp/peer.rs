// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{
    datagram::{UdpDatagram, UdpHeader},
    futures::UdpPopFuture,
    listener::SharedListener,
    socket::UdpSocket,
};
use crate::{
    fail::Fail,
    protocols::{
        arp,
        ethernet2::{
            frame::{EtherType2, Ethernet2Header},
            MacAddress,
        },
        ipv4::datagram::{Ipv4Header, Ipv4Protocol2},
        ipv4::Endpoint,
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

type OutgoingReq<T> = (Option<Endpoint>, Endpoint, T);
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

    sockets: HashMap<IoQueueDescriptor, UdpSocket>,
    bound: HashMap<Endpoint, SharedListener<RT::Buf>>,
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
                Self::do_send(rt.clone(), link_addr, buf, local, remote);
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
        // TODO: this should fail instead of panicking.
        assert_eq!(
            self.sockets.contains_key(&fd),
            false,
            "file descriptor in use"
        );

        // Create a detached socket and place it in our hashmap.
        // This socket will be bound to an address once we call do_bind.
        let socket: UdpSocket = UdpSocket::default();
        self.sockets.insert(fd, socket);
    }

    /// Binds a socket to a local endpoint address.
    pub fn do_bind(&mut self, fd: IoQueueDescriptor, addr: Endpoint) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::bind");

        // Local endpoint address in use.
        if self.bound.contains_key(&addr) {
            return Err(Fail::Malformed {
                details: "Port already listening",
            });
        }

        // Register local endpoint address.
        match self.sockets.get_mut(&fd) {
            Some(s) if s.get_local().is_none() => {
                s.set_local(Some(addr));
            }
            _ => return Err(Fail::BadFileDescriptor {}),
        }

        // Bind endpoint and register a listener for it.
        let listener = SharedListener::default();
        if self.bound.insert(addr, listener).is_some() {
            return Err(Fail::AddressInUse {});
        }

        Ok(())
    }

    /// Closes a UDP socket.
    pub fn do_close(&mut self, fd: IoQueueDescriptor) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::close");

        let socket: UdpSocket = match self.sockets.remove(&fd) {
            Some(s) => s,
            None => return Err(Fail::BadFileDescriptor {}),
        };

        // Remove endpoint binding.
        match socket.get_local() {
            Some(local) => {
                if self.bound.remove(&local).is_none() {
                    return Err(Fail::BadFileDescriptor {});
                }
            }
            None => return Err(Fail::BadFileDescriptor {}),
        }

        Ok(())
    }

    /// Consumes the payload from a buffer.
    pub fn do_receive(&mut self, ipv4_header: &Ipv4Header, buf: RT::Buf) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::receive");

        let (hdr, data) =
            UdpHeader::parse(ipv4_header, buf, self.rt.udp_options().get_rx_checksum())?;
        debug!("UDP received {:?}", hdr);

        let local: Endpoint = Endpoint::new(ipv4_header.dst_addr, hdr.dest_port());
        let remote: Option<Endpoint> = hdr
            .src_port()
            .map(|p| Endpoint::new(ipv4_header.src_addr, p));

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

    /// Pushes data to a remote UDP peer.
    pub fn do_pushto(&self, fd: IoQueueDescriptor, buf: RT::Buf, to: Endpoint) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("udp::pushto");

        let local: Option<Endpoint> = match self.sockets.get(&fd) {
            Some(s) if s.get_local().is_some() => s.get_local(),
            _ => return Err(Fail::BadFileDescriptor {}),
        };

        // Try to send the packet immediately.
        if let Some(link_addr) = self.arp.try_query(to.addr) {
            Self::do_send(self.rt.clone(), link_addr, buf, local, to);
        }
        // Defer send operation to the async path.
        else {
            self.outgoing.unbounded_send((local, to, buf)).unwrap()
        }

        Ok(())
    }

    /// Sends a UDP packet.
    fn do_send(
        rt: RT,
        link_addr: MacAddress,
        buf: RT::Buf,
        local: Option<Endpoint>,
        remote: Endpoint,
    ) {
        let udp_header = UdpHeader::new(local.map(|l| l.port), remote.port);
        debug!("UDP send {:?}", udp_header);
        let datagram = UdpDatagram::new(
            Ethernet2Header {
                dst_addr: link_addr,
                src_addr: rt.local_link_addr(),
                ether_type: EtherType2::Ipv4,
            },
            Ipv4Header::new(rt.local_ipv4_addr(), remote.addr, Ipv4Protocol2::Udp),
            udp_header,
            buf,
            rt.udp_options().get_tx_checksum(),
        );
        rt.transmit(datagram);
    }

    /// Pops data from a socket.
    pub fn do_pop(&self, fd: IoQueueDescriptor) -> UdpPopFuture<RT> {
        #[cfg(feature = "profiler")]
        timer!("udp::pop");

        let listener = match self.sockets.get(&fd) {
            Some(s) if s.get_local().is_some() => {
                Ok(self.bound.get(&s.get_local().unwrap()).unwrap().clone())
            }
            _ => Err(Fail::Malformed {
                details: "Invalid file descriptor",
            }),
        };

        UdpPopFuture::new(fd, listener)
    }
}
