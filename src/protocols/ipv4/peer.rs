// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail,
    protocols::{
        arp, icmpv4,
        ipv4::datagram::{Ipv4Header, Ipv4Protocol2},
        tcp,
        udp::UdpPeer,
    },
    runtime::Runtime,
};
use std::{future::Future, net::Ipv4Addr, time::Duration};

#[cfg(test)]
use crate::queue::IoQueueDescriptor;

pub struct Ipv4Peer<RT: Runtime> {
    rt: RT,
    icmpv4: icmpv4::Peer<RT>,
    pub tcp: tcp::Peer<RT>,
    pub udp: UdpPeer<RT>,
}

impl<RT: Runtime> Ipv4Peer<RT> {
    pub fn new(rt: RT, arp: arp::Peer<RT>) -> Ipv4Peer<RT> {
        let udp = UdpPeer::new(rt.clone(), arp.clone());
        let icmpv4 = icmpv4::Peer::new(rt.clone(), arp.clone());
        let tcp = tcp::Peer::new(rt.clone(), arp);
        Ipv4Peer {
            rt,
            icmpv4,
            tcp,
            udp,
        }
    }

    pub fn receive(&mut self, buf: RT::Buf) -> Result<(), Fail> {
        let (header, payload) = Ipv4Header::parse(buf)?;
        debug!("Ipv4 received {:?}", header);
        if header.dst_addr != self.rt.local_ipv4_addr() && !header.dst_addr.is_broadcast() {
            return Err(Fail::Misdelivered {});
        }
        match header.protocol {
            Ipv4Protocol2::Icmpv4 => self.icmpv4.receive(&header, payload),
            Ipv4Protocol2::Tcp => self.tcp.receive(&header, payload),
            Ipv4Protocol2::Udp => self.udp.do_receive(&header, payload),
        }
    }

    pub fn ping(
        &mut self,
        dest_ipv4_addr: Ipv4Addr,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<Duration, Fail>> {
        self.icmpv4.ping(dest_ipv4_addr, timeout)
    }
}

#[cfg(test)]
impl<RT: Runtime> Ipv4Peer<RT> {
    pub fn tcp_mss(&self, fd: IoQueueDescriptor) -> Result<usize, Fail> {
        self.tcp.remote_mss(fd)
    }

    pub fn tcp_rto(&self, fd: IoQueueDescriptor) -> Result<Duration, Fail> {
        self.tcp.current_rto(fd)
    }
}
