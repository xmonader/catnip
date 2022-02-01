// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{constants::FALLBACK_MSS, established::ControlBlock, SeqNumber};
use crate::{
    protocols::{
        arp::ArpPeer,
        ethernet2::{EtherType2, Ethernet2Header},
        ipv4::Ipv4Endpoint,
        ipv4::{Ipv4Header, Ipv4Protocol2},
        tcp::segment::{TcpHeader, TcpOptions2, TcpSegment},
    },
    runtime::Runtime,
};
use catwalk::SchedulerHandle;
use runtime::fail::Fail;
use runtime::RuntimeBuf;
use std::{
    cell::RefCell,
    convert::TryInto,
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};

struct ConnectResult<RT: Runtime> {
    waker: Option<Waker>,
    result: Option<Result<ControlBlock<RT>, Fail>>,
}

pub struct ActiveOpenSocket<RT: Runtime> {
    local_isn: SeqNumber,

    local: Ipv4Endpoint,
    remote: Ipv4Endpoint,

    rt: RT,
    arp: ArpPeer<RT>,

    #[allow(unused)]
    handle: SchedulerHandle,
    result: Rc<RefCell<ConnectResult<RT>>>,
}

impl<RT: Runtime> ActiveOpenSocket<RT> {
    pub fn new(
        local_isn: SeqNumber,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
        rt: RT,
        arp: ArpPeer<RT>,
    ) -> Self {
        let result = ConnectResult {
            waker: None,
            result: None,
        };
        let result = Rc::new(RefCell::new(result));

        let future = Self::background(
            local_isn,
            local,
            remote,
            rt.clone(),
            arp.clone(),
            result.clone(),
        );
        let handle = rt.spawn(future);

        // TODO: Add fast path here when remote is already in the ARP cache (and subtract one retry).
        Self {
            local_isn,
            local,
            remote,
            rt,
            arp,

            handle,
            result,
        }
    }

    pub fn poll_result(&mut self, context: &mut Context) -> Poll<Result<ControlBlock<RT>, Fail>> {
        let mut r = self.result.borrow_mut();
        match r.result.take() {
            None => {
                r.waker.replace(context.waker().clone());
                Poll::Pending
            }
            Some(r) => Poll::Ready(r),
        }
    }

    fn set_result(&mut self, result: Result<ControlBlock<RT>, Fail>) {
        let mut r = self.result.borrow_mut();
        if let Some(w) = r.waker.take() {
            w.wake()
        }
        r.result.replace(result);
    }

    pub fn receive(&mut self, header: &TcpHeader) {
        let expected_seq = self.local_isn + SeqNumber::from(1);

        // Bail if we didn't receive a ACK packet with the right sequence number.
        if !(header.ack && header.ack_num == expected_seq) {
            return;
        }

        // Check if our peer is refusing our connection request.
        if header.rst {
            self.set_result(Err(Fail::ConnectionRefused {}));
            return;
        }

        // Bail if we didn't receive a SYN packet.
        if !header.syn {
            return;
        }

        debug!("Received SYN+ACK: {:?}", header);

        // Acknowledge the SYN+ACK segment.
        let remote_link_addr = match self.arp.try_query(self.remote.get_address()) {
            Some(r) => r,
            None => panic!("TODO: Clean up ARP query control flow"),
        };
        let remote_seq_num = header.seq_num + SeqNumber::from(1);

        let tcp_options = self.rt.tcp_options();

        let mut tcp_hdr = TcpHeader::new(self.local.get_port(), self.remote.get_port());
        tcp_hdr.ack = true;
        tcp_hdr.ack_num = remote_seq_num;
        tcp_hdr.window_size = tcp_options.receive_window_size();
        tcp_hdr.seq_num = self.local_isn + SeqNumber::from(1);
        debug!("Sending ACK: {:?}", tcp_hdr);

        let segment = TcpSegment {
            ethernet2_hdr: Ethernet2Header::new(
                remote_link_addr,
                self.rt.local_link_addr(),
                EtherType2::Ipv4,
            ),
            ipv4_hdr: Ipv4Header::new(
                self.local.get_address(),
                self.remote.get_address(),
                Ipv4Protocol2::Tcp,
            ),
            tcp_hdr,
            data: RT::Buf::empty(),
            tx_checksum_offload: tcp_options.tx_checksum_offload(),
        };
        self.rt.transmit(segment);

        let mut remote_window_scale = None;
        let mut mss = FALLBACK_MSS;
        for option in header.iter_options() {
            match option {
                TcpOptions2::WindowScale(w) => {
                    info!("Received window scale: {}", w);
                    remote_window_scale = Some(*w);
                }
                TcpOptions2::MaximumSegmentSize(m) => {
                    info!("Received advertised MSS: {}", m);
                    mss = *m as usize;
                }
                _ => continue,
            }
        }

        let (local_window_scale, remote_window_scale) = match remote_window_scale {
            Some(w) => (tcp_options.window_scale() as u32, w),
            None => (0, 0),
        };

        // TODO(RFC1323): Clamp the scale to 14 instead of panicking.
        assert!(local_window_scale <= 14 && remote_window_scale <= 14);

        let rx_window_size: u32 = (tcp_options.receive_window_size())
            .checked_shl(local_window_scale as u32)
            .expect("TODO: Window size overflow")
            .try_into()
            .expect("TODO: Window size overflow");

        let tx_window_size: u32 = (header.window_size)
            .checked_shl(remote_window_scale as u32)
            .expect("TODO: Window size overflow")
            .try_into()
            .expect("TODO: Window size overflow");

        info!(
            "Window sizes: local {}, remote {}",
            rx_window_size, tx_window_size
        );
        info!(
            "Window scale: local {}, remote {}",
            local_window_scale, remote_window_scale
        );

        let cb = ControlBlock::new(
            self.local,
            self.remote,
            self.rt.clone(),
            self.arp.clone(),
            remote_seq_num,
            self.rt.tcp_options().ack_delay_timeout(),
            rx_window_size,
            local_window_scale,
            expected_seq,
            tx_window_size,
            remote_window_scale,
            mss,
            tcp_options.congestion_ctrl_type(),
            tcp_options.congestion_ctrl_options(),
        );
        self.set_result(Ok(cb));
    }

    fn background(
        local_isn: SeqNumber,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
        rt: RT,
        arp: ArpPeer<RT>,
        result: Rc<RefCell<ConnectResult<RT>>>,
    ) -> impl Future<Output = ()> {
        let tcp_options = rt.tcp_options();
        let handshake_retries: usize = tcp_options.handshake_retries();
        let handshake_timeout = tcp_options.handshake_timeout();

        async move {
            for _ in 0..handshake_retries {
                let remote_link_addr = match arp.query(remote.get_address()).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("ARP query failed: {:?}", e);
                        continue;
                    }
                };

                let mut tcp_hdr = TcpHeader::new(local.get_port(), remote.get_port());
                tcp_hdr.syn = true;
                tcp_hdr.seq_num = local_isn;
                tcp_hdr.window_size = tcp_options.receive_window_size();

                let mss = tcp_options.advertised_mss() as u16;
                tcp_hdr.push_option(TcpOptions2::MaximumSegmentSize(mss));
                info!("Advertising MSS: {}", mss);

                tcp_hdr.push_option(TcpOptions2::WindowScale(tcp_options.window_scale()));
                info!("Advertising window scale: {}", tcp_options.window_scale());

                debug!("Sending SYN {:?}", tcp_hdr);
                let segment = TcpSegment {
                    ethernet2_hdr: Ethernet2Header::new(
                        remote_link_addr,
                        rt.local_link_addr(),
                        EtherType2::Ipv4,
                    ),
                    ipv4_hdr: Ipv4Header::new(
                        local.get_address(),
                        remote.get_address(),
                        Ipv4Protocol2::Tcp,
                    ),
                    tcp_hdr,
                    data: RT::Buf::empty(),
                    tx_checksum_offload: tcp_options.tx_checksum_offload(),
                };
                rt.transmit(segment);
                rt.wait(handshake_timeout).await;
            }
            let mut r = result.borrow_mut();
            if let Some(w) = r.waker.take() {
                w.wake()
            }
            r.result.replace(Err(Fail::Timeout {}));
        }
    }
}
