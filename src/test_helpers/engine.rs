// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail,
    protocols::{
        arp,
        ethernet2::{
            frame::{EtherType2, Ethernet2Header},
            MacAddress,
        },
        ipv4,
        tcp::operations::{AcceptFuture, ConnectFuture, PopFuture, PushFuture},
        udp::UdpPopFuture,
    },
    queue::IoQueueType,
    queue::{IoQueueDescriptor, IoQueueTable},
    runtime::Runtime,
};

use std::{collections::HashMap, future::Future, net::Ipv4Addr, time::Duration};
pub struct Engine<RT: Runtime> {
    rt: RT,
    pub arp: arp::Peer<RT>,
    pub ipv4: ipv4::Peer<RT>,
    pub file_table: IoQueueTable,
}

impl<RT: Runtime> Engine<RT> {
    pub fn new(rt: RT) -> Result<Self, Fail> {
        let now = rt.now();
        let file_table = IoQueueTable::new();
        let arp = arp::Peer::new(now, rt.clone(), rt.arp_options())?;
        let ipv4 = ipv4::Peer::new(rt.clone(), arp.clone());
        Ok(Engine {
            rt,
            arp,
            ipv4,
            file_table,
        })
    }

    pub fn rt(&mut self) -> &mut RT {
        &mut self.rt
    }

    pub fn receive(&mut self, bytes: RT::Buf) -> Result<(), Fail> {
        let (header, payload) = Ethernet2Header::parse(bytes)?;
        debug!("Engine received {:?}", header);
        if self.rt.local_link_addr() != header.dst_addr && !header.dst_addr.is_broadcast() {
            return Err(Fail::Ignored {
                details: "Physical dst_addr mismatch",
            });
        }
        match header.ether_type {
            EtherType2::Arp => self.arp.receive(payload),
            EtherType2::Ipv4 => self.ipv4.receive(payload),
        }
    }

    pub fn ipv4_ping(
        &mut self,
        dest_ipv4_addr: Ipv4Addr,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<Duration, Fail>> {
        self.ipv4.ping(dest_ipv4_addr, timeout)
    }

    pub fn udp_pushto(
        &self,
        fd: IoQueueDescriptor,
        buf: RT::Buf,
        to: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        self.ipv4.udp.do_pushto(fd, buf, to)
    }

    pub fn udp_pop(&mut self, fd: IoQueueDescriptor) -> UdpPopFuture<RT> {
        self.ipv4.udp.do_pop(fd)
    }

    pub fn udp_socket(&mut self) -> Result<IoQueueDescriptor, Fail> {
        let fd = self.file_table.alloc(IoQueueType::UdpSocket);
        self.ipv4.udp.do_socket(fd);
        Ok(fd)
    }

    pub fn udp_bind(
        &mut self,
        socket_fd: IoQueueDescriptor,
        endpoint: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        self.ipv4.udp.do_bind(socket_fd, endpoint)
    }

    pub fn udp_close(&mut self, socket_fd: IoQueueDescriptor) -> Result<(), Fail> {
        self.ipv4.udp.do_close(socket_fd)
    }

    pub fn tcp_socket(&mut self) -> Result<IoQueueDescriptor, Fail> {
        let fd = self.file_table.alloc(IoQueueType::TcpSocket);
        self.ipv4.tcp.do_socket(fd);
        Ok(fd)
    }

    pub fn tcp_connect(
        &mut self,
        socket_fd: IoQueueDescriptor,
        remote_endpoint: ipv4::Endpoint,
    ) -> ConnectFuture<RT> {
        self.ipv4.tcp.connect(socket_fd, remote_endpoint)
    }

    pub fn tcp_bind(
        &mut self,
        socket_fd: IoQueueDescriptor,
        endpoint: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        self.ipv4.tcp.bind(socket_fd, endpoint)
    }

    pub fn tcp_accept(&mut self, fd: IoQueueDescriptor) -> AcceptFuture<RT> {
        let newfd = self.file_table.alloc(IoQueueType::TcpSocket);
        self.ipv4.tcp.do_accept(fd, newfd)
    }

    pub fn tcp_push(&mut self, socket_fd: IoQueueDescriptor, buf: RT::Buf) -> PushFuture<RT> {
        self.ipv4.tcp.push(socket_fd, buf)
    }

    pub fn tcp_pop(&mut self, socket_fd: IoQueueDescriptor) -> PopFuture<RT> {
        self.ipv4.tcp.pop(socket_fd)
    }

    pub fn tcp_close(&mut self, socket_fd: IoQueueDescriptor) -> Result<(), Fail> {
        self.ipv4.tcp.do_close(socket_fd)
    }

    pub fn tcp_listen(&mut self, socket_fd: IoQueueDescriptor, backlog: usize) -> Result<(), Fail> {
        self.ipv4.tcp.listen(socket_fd, backlog)
    }

    pub fn arp_query(&self, ipv4_addr: Ipv4Addr) -> impl Future<Output = Result<MacAddress, Fail>> {
        self.arp.query(ipv4_addr)
    }

    pub fn tcp_mss(&self, handle: IoQueueDescriptor) -> Result<usize, Fail> {
        self.ipv4.tcp_mss(handle)
    }

    pub fn tcp_rto(&self, handle: IoQueueDescriptor) -> Result<Duration, Fail> {
        self.ipv4.tcp_rto(handle)
    }

    pub fn export_arp_cache(&self) -> HashMap<Ipv4Addr, MacAddress> {
        self.arp.export_cache()
    }
}
