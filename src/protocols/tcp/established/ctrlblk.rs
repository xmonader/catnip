// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{
    sender::congestion_ctrl,
    sender::Sender,
    sender::{congestion_ctrl::CongestionControlConstructor, UnackedSegment},
};

use crate::{
    collections::watched::{WatchFuture, WatchedValue},
    fail::Fail,
    protocols::{
        arp,
        ethernet2::{
            frame::{EtherType2, Ethernet2Header},
            MacAddress,
        },
        ipv4,
        ipv4::datagram::{Ipv4Header, Ipv4Protocol2},
        tcp::{
            segment::{TcpHeader, TcpSegment},
            SeqNumber,
        },
    },
    runtime::Runtime,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    convert::TryInto,
    num::Wrapping,
    rc::Rc,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

const RECV_QUEUE_SZ: usize = 2048;
const MAX_OUT_OF_ORDER: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Established,
    ActiveClose,
    FinWait1,
    FinWait2,
    FinWait3,
    Closing1,
    Closing2,
    TimeWait1,
    TimeWait2,
    PassiveClose,
    CloseWait1,
    CloseWait2,
    LastAck,
    Closed,
    Reset,
}

struct ReceiveQueue<RT: Runtime> {
    //                     |-----------------recv_window-------------------|
    //                base_seq_no             ack_seq_no             recv_seq_no
    //                     v                       v                       v
    // ... ----------------|-----------------------|-----------------------| (unavailable)
    //         received           acknowledged           unacknowledged
    //
    // NB: We can have `ack_seq_no < base_seq_no` when the application fully drains the receive
    // buffer before we've sent a pure ACK or transmitted some data on which we could piggyback
    // an ACK. The sender, however, will still be computing the receive window relative to the
    // the old `ack_seq_no` until we send them an ACK (see the diagram in sender.rs).
    //
    pub base_seq_no: WatchedValue<SeqNumber>,
    pub ack_seq_no: WatchedValue<SeqNumber>,
    pub recv_seq_no: WatchedValue<SeqNumber>,
    recv_queue: RefCell<VecDeque<RT::Buf>>,
}

impl<RT: Runtime> ReceiveQueue<RT> {
    pub fn new(base_seq_no: SeqNumber, ack_seq_no: SeqNumber, recv_seq_no: SeqNumber) -> Self {
        Self {
            base_seq_no: WatchedValue::new(base_seq_no),
            ack_seq_no: WatchedValue::new(ack_seq_no),
            recv_seq_no: WatchedValue::new(recv_seq_no),
            recv_queue: RefCell::new(VecDeque::with_capacity(RECV_QUEUE_SZ)),
        }
    }

    pub fn pop(&self) -> Option<RT::Buf> {
        let buf: RT::Buf = self.recv_queue.borrow_mut().pop_front()?;
        self.base_seq_no.modify(|b| b + Wrapping(buf.len() as u32));

        Some(buf)
    }

    pub fn push(&self, buf: RT::Buf) {
        let buf_len: u32 = buf.len() as u32;
        self.recv_queue.borrow_mut().push_back(buf);
        self.recv_seq_no.modify(|r| r + Wrapping(buf_len as u32));
    }

    pub fn size(&self) -> usize {
        self.recv_queue
            .borrow()
            .iter()
            .map(|b| b.len())
            .sum::<usize>()
    }
}

/// Transmission control block for representing our TCP connection.
pub struct ControlBlock<RT: Runtime> {
    local: ipv4::Endpoint,
    remote: ipv4::Endpoint,

    rt: Rc<RT>,
    arp: Rc<arp::Peer<RT>>,

    /// The sender end of our connection.
    sender: Sender<RT>,

    state: WatchedValue<State>,

    //                     |-----------------recv_window-------------------|
    //                base_seq_no             ack_seq_no             recv_seq_no
    //                     v                       v                       v
    // ... ----------------|-----------------------|-----------------------| (unavailable)
    //         received           acknowledged           unacknowledged
    //
    // NB: We can have `ack_seq_no < base_seq_no` when the application fully drains the receive
    // buffer before we've sent a pure ACK or transmitted some data on which we could piggyback
    // an ACK. The sender, however, will still be computing the receive window relative to the
    // the old `ack_seq_no` until we send them an ACK (see the diagram in sender.rs).
    //
    /// Timeout for delayed ACKs.
    ack_delay_timeout: Duration,

    ack_deadline: WatchedValue<Option<Instant>>,

    max_window_size: u32,
    window_scale: u32,

    waker: RefCell<Option<Waker>>,
    out_of_order: RefCell<BTreeMap<SeqNumber, RT::Buf>>,

    receive_queue: ReceiveQueue<RT>,
}

//==============================================================================

impl<RT: Runtime> ControlBlock<RT> {
    pub fn new(
        local: ipv4::Endpoint,
        remote: ipv4::Endpoint,
        rt: RT,
        arp: arp::Peer<RT>,
        receiver_seq_no: SeqNumber,
        ack_delay_timeout: Duration,
        receiver_window_size: u32,
        receiver_window_scale: u32,
        sender_seq_no: SeqNumber,
        sender_window_size: u32,
        sender_window_scale: u8,
        sender_mss: usize,
        sender_cc_constructor: CongestionControlConstructor<RT>,
        sender_congestion_control_options: Option<congestion_ctrl::Options>,
    ) -> Self {
        let sender = Sender::new(
            sender_seq_no,
            sender_window_size,
            sender_window_scale,
            sender_mss,
            sender_cc_constructor,
            sender_congestion_control_options,
        );
        Self {
            local,
            remote,
            rt: Rc::new(rt),
            arp: Rc::new(arp),
            sender: sender,
            state: WatchedValue::new(State::Established),
            ack_delay_timeout,
            ack_deadline: WatchedValue::new(None),
            max_window_size: receiver_window_size,
            window_scale: receiver_window_scale,
            waker: RefCell::new(None),
            out_of_order: RefCell::new(BTreeMap::new()),
            receive_queue: ReceiveQueue::new(receiver_seq_no, receiver_seq_no, receiver_seq_no),
        }
    }

    pub fn get_state(&self) -> (State, WatchFuture<State>) {
        self.state.watch()
    }

    pub fn set_state(&self, new_value: State) {
        self.state.set(new_value)
    }

    pub fn get_local(&self) -> ipv4::Endpoint {
        self.local
    }

    pub fn get_remote(&self) -> ipv4::Endpoint {
        self.remote
    }

    pub fn rt(&self) -> Rc<RT> {
        self.rt.clone()
    }

    pub fn arp(&self) -> Rc<arp::Peer<RT>> {
        self.arp.clone()
    }

    pub fn send(&self, buf: RT::Buf) -> Result<(), Fail> {
        if self.state.get() != State::Established {
            return Err(Fail::Ignored {
                details: "Sender closed",
            });
        }

        self.sender.send(buf, self)
    }

    pub fn congestion_ctrl_watch_retransmit_now_flag(&self) -> (bool, WatchFuture<bool>) {
        self.sender.congestion_ctrl_watch_retransmit_now_flag()
    }

    pub fn congestion_ctrl_on_fast_retransmit(&self) {
        self.sender.congestion_ctrl_on_fast_retransmit()
    }

    pub fn congestion_ctrl_on_rto(&self, base_seq_no: Wrapping<u32>) {
        self.sender.congestion_ctrl_on_rto(base_seq_no)
    }

    pub fn congestion_ctrl_on_send(&self, rto: Duration, num_sent_bytes: u32) {
        self.sender.congestion_ctrl_on_send(rto, num_sent_bytes)
    }

    pub fn congestion_ctrl_on_cwnd_check_before_send(&self) {
        self.sender.congestion_ctrl_on_cwnd_check_before_send()
    }

    pub fn congestion_ctrl_watch_cwnd(&self) -> (u32, WatchFuture<u32>) {
        self.sender.congestion_ctrl_watch_cwnd()
    }

    pub fn congestion_ctrl_watch_limited_transmit_cwnd_increase(&self) -> (u32, WatchFuture<u32>) {
        self.sender
            .congestion_ctrl_watch_limited_transmit_cwnd_increase()
    }

    pub fn get_mss(&self) -> usize {
        self.sender.get_mss()
    }

    pub fn get_window_size(&self) -> (u32, WatchFuture<u32>) {
        self.sender.get_window_size()
    }

    pub fn get_base_seq_no(&self) -> (Wrapping<u32>, WatchFuture<Wrapping<u32>>) {
        self.sender.get_base_seq_no()
    }

    pub fn get_unsent_seq_no(&self) -> (Wrapping<u32>, WatchFuture<Wrapping<u32>>) {
        self.sender.get_unsent_seq_no()
    }

    pub fn get_sent_seq_no(&self) -> (Wrapping<u32>, WatchFuture<Wrapping<u32>>) {
        self.sender.get_sent_seq_no()
    }

    pub fn modify_sent_seq_no(&self, f: impl FnOnce(Wrapping<u32>) -> Wrapping<u32>) {
        self.sender.modify_sent_seq_no(f)
    }

    pub fn get_retransmit_deadline(&self) -> (Option<Instant>, WatchFuture<Option<Instant>>) {
        self.sender.get_retransmit_deadline()
    }

    pub fn set_retransmit_deadline(&self, when: Option<Instant>) {
        self.sender.set_retransmit_deadline(when);
    }

    pub fn pop_unacked_segment(&self) -> Option<UnackedSegment<RT>> {
        self.sender.pop_unacked_segment()
    }

    pub fn push_unacked_segment(&self, segment: UnackedSegment<RT>) {
        self.sender.push_unacked_segment(segment)
    }

    pub fn rto_estimate(&self) -> Duration {
        self.sender.rto_estimate()
    }

    pub fn rto_record_failure(&self) {
        self.sender.rto_record_failure()
    }

    pub fn unsent_top_size(&self) -> Option<usize> {
        self.sender.top_size_unsent()
    }

    pub fn pop_unsent_segment(&self, max_bytes: usize) -> Option<RT::Buf> {
        self.sender.pop_unsent(max_bytes)
    }

    pub fn pop_one_unsent_byte(&self) -> Option<RT::Buf> {
        self.sender.pop_one_unsent_byte()
    }

    pub fn receive(&self, header: &TcpHeader, data: RT::Buf) {
        debug!(
            "{:?} Connection Receiving {} bytes + {:?}",
            self.state.get(),
            data.len(),
            header
        );
        let now = self.rt.now();
        if header.syn {
            warn!("Ignoring duplicate SYN on established connection");
        }
        if header.rst {
            self.state.set(State::Reset);
        }
        if header.fin && header.ack {
            match self.state.get() {
                State::FinWait1 => self.state.set(State::TimeWait1),
                s => panic!("bad peer state {:?}", s),
            }
        } else {
            if header.fin {
                match self.state.get() {
                    State::FinWait1 => self.state.set(State::Closing1),
                    State::FinWait2 => self.state.set(State::FinWait3),
                    State::Established => self.state.set(State::PassiveClose),
                    s => panic!("bad peer state {:?}", s),
                }
            }
            if header.ack {
                match self.state.get() {
                    State::FinWait1 => self.state.set(State::FinWait2),
                    State::Closing2 => self.state.set(State::TimeWait2),
                    State::Established => {
                        if let Err(e) = self.sender.remote_ack(header.ack_num, now) {
                            warn!("Ignoring remote ack for {:?}: {:?}", header, e);
                        }
                    }
                    State::LastAck => self.state.set(State::Closed),
                    s => panic!("bad peer state {:?}", s),
                }
            }
        }
        if self.state.get() == State::Established {
            if let Err(e) = self.sender.update_remote_window(header.window_size as u16) {
                warn!("Invalid window size update for {:?}: {:?}", header, e);
            }
        }
        if !data.is_empty() {
            if self.state.get() != State::Established {
                warn!("Receiver closed");
            }
            if let Err(e) = self.receive_data(header.seq_num, data, now) {
                warn!("Ignoring remote data for {:?}: {:?}", header, e);
            }
        }
    }

    pub fn close(&self) -> Result<(), Fail> {
        match self.state.get() {
            State::Established => self.state.set(State::ActiveClose),
            State::CloseWait1 => self.state.set(State::CloseWait2),
            s => panic!("bad state {:?}", s),
        }

        Ok(())
    }

    /// Fetch a TCP header filling out various values based on our current state.
    pub fn tcp_header(&self) -> TcpHeader {
        let mut header = TcpHeader::new(self.local.port, self.remote.port);
        header.window_size = self.hdr_window_size();

        // Check if we have acknowledged all bytes that we have received. If not, piggy back an ACK
        // on this message.
        if self.state.get() != State::CloseWait2 {
            if let Some(ack_seq_no) = self.current_ack() {
                header.ack_num = ack_seq_no;
                header.ack = true;
            }
        }
        header
    }

    /// Transmit this message to our connected peer.
    pub fn emit(&self, header: TcpHeader, data: RT::Buf, remote_link_addr: MacAddress) {
        if header.ack {
            let (recv_seq_no, _) = self.get_recv_seq_no();
            if self.state.get() == State::PassiveClose || self.state.get() == State::FinWait3 {
                assert_eq!(header.ack_num, recv_seq_no + Wrapping(1));
            } else {
                assert_eq!(header.ack_num, recv_seq_no);
            }
            self.set_ack_deadline(None);
            self.set_ack_seq_no(header.ack_num);
        }

        debug!("Sending {} bytes + {:?}", data.len(), header);
        let segment = TcpSegment {
            ethernet2_hdr: Ethernet2Header {
                dst_addr: remote_link_addr,
                src_addr: self.rt.local_link_addr(),
                ether_type: EtherType2::Ipv4,
            },
            ipv4_hdr: Ipv4Header::new(self.local.addr, self.remote.addr, Ipv4Protocol2::Tcp),
            tcp_hdr: header,
            data,
            tx_checksum_offload: self.rt.tcp_options().tx_checksum_offload(),
        };
        self.rt.transmit(segment);
    }

    pub fn remote_mss(&self) -> usize {
        self.sender.remote_mss()
    }

    pub fn rto_current(&self) -> Duration {
        self.sender.current_rto()
    }

    pub fn get_ack_seq_no(&self) -> (Wrapping<u32>, WatchFuture<Wrapping<u32>>) {
        let (seq_no, fut) = self.receive_queue.ack_seq_no.watch();
        (seq_no, fut)
    }

    pub fn set_ack_seq_no(&self, new_value: Wrapping<u32>) {
        self.receive_queue.ack_seq_no.set(new_value)
    }

    pub fn get_recv_seq_no(&self) -> (Wrapping<u32>, WatchFuture<Wrapping<u32>>) {
        self.receive_queue.recv_seq_no.watch()
    }

    pub fn get_ack_deadline(&self) -> (Option<Instant>, WatchFuture<Option<Instant>>) {
        self.ack_deadline.watch()
    }

    pub fn set_ack_deadline(&self, when: Option<Instant>) {
        self.ack_deadline.set(when);
    }

    pub fn hdr_window_size(&self) -> u16 {
        let Wrapping(bytes_outstanding) =
            self.receive_queue.recv_seq_no.get() - self.receive_queue.base_seq_no.get();
        let window_size = self.max_window_size - bytes_outstanding;
        let hdr_window_size = (window_size >> self.window_scale)
            .try_into()
            .expect("Window size overflow");
        debug!(
            "Sending window size update -> {} (hdr {}, scale {})",
            (hdr_window_size as u32) << self.window_scale,
            hdr_window_size,
            self.window_scale
        );
        hdr_window_size
    }

    /// Returns the ack sequence number to use for the next packet based on all the bytes we have
    /// received. This ack sequence number will be piggy backed on the next packet send.
    /// If all received bytes have been acknowledged returns None.
    pub fn current_ack(&self) -> Option<SeqNumber> {
        let ack_seq_no = self.receive_queue.ack_seq_no.get();
        let recv_seq_no = self.receive_queue.recv_seq_no.get();

        // It is okay if ack_seq_no is greater than the seq number. This can happen when we have
        // ACKed a FIN so our ACK number is +1 greater than our seq number.
        if ack_seq_no == recv_seq_no {
            None
        } else {
            Some(recv_seq_no)
        }
    }

    pub fn poll_recv(&self, ctx: &mut Context) -> Poll<Result<RT::Buf, Fail>> {
        if self.receive_queue.base_seq_no.get() == self.receive_queue.recv_seq_no.get() {
            *self.waker.borrow_mut() = Some(ctx.waker().clone());
            return Poll::Pending;
        }

        let segment = self
            .receive_queue
            .pop()
            .expect("recv_seq > base_seq without data in queue?");

        Poll::Ready(Ok(segment))
    }

    pub fn receive_data(&self, seq_no: SeqNumber, buf: RT::Buf, now: Instant) -> Result<(), Fail> {
        let recv_seq_no = self.receive_queue.recv_seq_no.get();
        if seq_no > recv_seq_no {
            let mut out_of_order = self.out_of_order.borrow_mut();
            if !out_of_order.contains_key(&seq_no) {
                while out_of_order.len() > MAX_OUT_OF_ORDER {
                    let (&key, _) = out_of_order.iter().rev().next().unwrap();
                    out_of_order.remove(&key);
                }
                out_of_order.insert(seq_no, buf);
                return Err(Fail::Ignored {
                    details: "Out of order segment (reordered)",
                });
            }
        }
        if seq_no < recv_seq_no {
            return Err(Fail::Ignored {
                details: "Out of order segment (duplicate)",
            });
        }

        let unread_bytes: usize = self.receive_queue.size();
        if unread_bytes + buf.len() > self.max_window_size as usize {
            return Err(Fail::Ignored {
                details: "Full receive window",
            });
        }

        self.receive_queue.push(buf);
        if let Some(w) = self.waker.borrow_mut().take() {
            w.wake()
        }

        // TODO: How do we handle when the other side is in PERSIST state here?
        if self.ack_deadline.get().is_none() {
            self.ack_deadline.set(Some(now + self.ack_delay_timeout));
        }

        let new_recv_seq_no = self.receive_queue.recv_seq_no.get();
        let old_data = {
            let mut out_of_order = self.out_of_order.borrow_mut();
            out_of_order.remove(&new_recv_seq_no)
        };
        if let Some(old_data) = old_data {
            info!("Recovering out-of-order packet at {}", new_recv_seq_no);
            if let Err(e) = self.receive_data(new_recv_seq_no, old_data, now) {
                info!("Failed to recover out-of-order packet: {:?}", e);
            }
        }

        Ok(())
    }
}
