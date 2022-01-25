// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    collections::bytes::{Bytes, BytesMut},
    futures::operation::FutureOperation,
    interop::{dmtr_sgarray_t, dmtr_sgaseg_t},
    logging,
    protocols::{arp::ArpOptions, ethernet2::MacAddress, tcp, udp},
    runtime::{PacketBuf, Runtime, RECEIVE_BATCH_SIZE},
    scheduler::{Scheduler, SchedulerHandle},
    test_helpers::Engine,
    timer::{Timer, TimerRc},
};
use arrayvec::ArrayVec;
use futures::FutureExt;
use rand::{
    distributions::{Distribution, Standard},
    rngs::SmallRng,
    seq::SliceRandom,
    Rng, SeedableRng,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    mem,
    net::Ipv4Addr,
    ptr,
    rc::Rc,
    slice,
    time::{Duration, Instant},
};

//==============================================================================
// Types
//==============================================================================

pub type TestEngine = Engine<TestRuntime>;

//==============================================================================
// Structures
//==============================================================================

// TODO: Drop inner mutability pattern.
pub struct Inner {
    #[allow(unused)]
    timer: TimerRc,
    rng: SmallRng,
    incoming: VecDeque<Bytes>,
    outgoing: VecDeque<Bytes>,
}

#[derive(Clone)]
pub struct TestRuntime {
    name: &'static str,
    link_addr: MacAddress,
    ipv4_addr: Ipv4Addr,
    arp_options: ArpOptions,
    udp_options: udp::UdpConfig,
    tcp_options: tcp::Options<TestRuntime>,
    inner: Rc<RefCell<Inner>>,
    scheduler: Scheduler<FutureOperation<TestRuntime>>,
}

//==============================================================================
// Associate Functions
//==============================================================================

impl TestRuntime {
    pub fn new(
        name: &'static str,
        now: Instant,
        arp_options: ArpOptions,
        udp_options: udp::UdpConfig,
        tcp_options: tcp::Options<TestRuntime>,
        link_addr: MacAddress,
        ipv4_addr: Ipv4Addr,
    ) -> Self {
        logging::initialize();

        let inner = Inner {
            timer: TimerRc(Rc::new(Timer::new(now))),
            rng: SmallRng::from_seed([0; 32]),
            incoming: VecDeque::new(),
            outgoing: VecDeque::new(),
        };
        Self {
            name,
            link_addr,
            ipv4_addr,
            inner: Rc::new(RefCell::new(inner)),
            scheduler: Scheduler::new(),
            arp_options,
            udp_options,
            tcp_options,
        }
    }

    pub fn pop_frame(&self) -> Bytes {
        self.inner.borrow_mut().outgoing.pop_front().unwrap()
    }

    pub fn pop_frame_unchecked(&self) -> Option<Bytes> {
        self.inner.borrow_mut().outgoing.pop_front()
    }

    pub fn push_frame(&self, buf: Bytes) {
        self.inner.borrow_mut().incoming.push_back(buf);
    }

    pub fn poll_scheduler(&self) {
        // let mut ctx = Context::from_waker(noop_waker_ref());
        self.scheduler.poll();
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

impl Runtime for TestRuntime {
    type Buf = Bytes;
    type WaitFuture = crate::timer::WaitFuture<TimerRc>;

    fn into_sgarray(&self, buf: Bytes) -> dmtr_sgarray_t {
        let buf_copy: Box<[u8]> = (&buf[..]).into();
        let ptr = Box::into_raw(buf_copy);
        let sgaseg = dmtr_sgaseg_t {
            sgaseg_buf: ptr as *mut _,
            sgaseg_len: buf.len() as u32,
        };
        dmtr_sgarray_t {
            sga_buf: ptr::null_mut(),
            sga_numsegs: 1,
            sga_segs: [sgaseg],
            sga_addr: unsafe { mem::zeroed() },
        }
    }

    fn alloc_sgarray(&self, size: usize) -> dmtr_sgarray_t {
        let allocation: Box<[u8]> = unsafe { Box::new_uninit_slice(size).assume_init() };
        let ptr = Box::into_raw(allocation);
        let sgaseg = dmtr_sgaseg_t {
            sgaseg_buf: ptr as *mut _,
            sgaseg_len: size as u32,
        };
        dmtr_sgarray_t {
            sga_buf: ptr::null_mut(),
            sga_numsegs: 1,
            sga_segs: [sgaseg],
            sga_addr: unsafe { mem::zeroed() },
        }
    }

    fn free_sgarray(&self, sga: dmtr_sgarray_t) {
        assert_eq!(sga.sga_numsegs, 1);
        let sgaseg = sga.sga_segs[0];
        let allocation: Box<[u8]> = unsafe {
            Box::from_raw(slice::from_raw_parts_mut(
                sgaseg.sgaseg_buf as *mut _,
                sgaseg.sgaseg_len as usize,
            ))
        };
        drop(allocation);
    }

    fn clone_sgarray(&self, sga: &dmtr_sgarray_t) -> Bytes {
        let mut len = 0;
        for i in 0..sga.sga_numsegs as usize {
            len += sga.sga_segs[i].sgaseg_len;
        }
        let mut buf = BytesMut::zeroed(len as usize).unwrap();
        let mut pos = 0;
        for i in 0..sga.sga_numsegs as usize {
            let seg = &sga.sga_segs[i];
            let seg_slice = unsafe {
                slice::from_raw_parts(seg.sgaseg_buf as *mut u8, seg.sgaseg_len as usize)
            };
            buf[pos..(pos + seg_slice.len())].copy_from_slice(seg_slice);
            pos += seg_slice.len();
        }
        buf.freeze()
    }

    fn transmit(&self, pkt: impl PacketBuf<Bytes>) {
        let header_size = pkt.header_size();
        let body_size = pkt.body_size();

        let mut buf = BytesMut::zeroed(header_size + body_size).unwrap();
        pkt.write_header(&mut buf[..header_size]);
        if let Some(body) = pkt.take_body() {
            buf[header_size..].copy_from_slice(&body[..]);
        }
        self.inner.borrow_mut().outgoing.push_back(buf.freeze());
    }

    fn receive(&self) -> ArrayVec<Bytes, RECEIVE_BATCH_SIZE> {
        let mut out = ArrayVec::new();
        if let Some(buf) = self.inner.borrow_mut().incoming.pop_front() {
            out.push(buf);
        }
        out
    }

    fn scheduler(&self) -> &Scheduler<FutureOperation<Self>> {
        &self.scheduler
    }

    fn local_link_addr(&self) -> MacAddress {
        self.link_addr
    }

    fn local_ipv4_addr(&self) -> Ipv4Addr {
        self.ipv4_addr
    }

    fn tcp_options(&self) -> tcp::Options<TestRuntime> {
        self.tcp_options.clone()
    }

    fn udp_options(&self) -> udp::UdpConfig {
        self.udp_options.clone()
    }

    fn arp_options(&self) -> ArpOptions {
        self.arp_options.clone()
    }

    fn advance_clock(&self, now: Instant) {
        self.inner.borrow_mut().timer.0.advance_clock(now);
    }

    fn wait(&self, duration: Duration) -> Self::WaitFuture {
        let inner = self.inner.borrow_mut();
        let now = inner.timer.0.now();
        inner
            .timer
            .0
            .wait_until(inner.timer.clone(), now + duration)
    }

    fn wait_until(&self, when: Instant) -> Self::WaitFuture {
        let inner = self.inner.borrow_mut();
        inner.timer.0.wait_until(inner.timer.clone(), when)
    }

    fn now(&self) -> Instant {
        self.inner.borrow().timer.0.now()
    }

    fn rng_gen<T>(&self) -> T
    where
        Standard: Distribution<T>,
    {
        let mut inner = self.inner.borrow_mut();
        inner.rng.gen()
    }

    fn rng_shuffle<T>(&self, slice: &mut [T]) {
        let mut inner = self.inner.borrow_mut();
        slice.shuffle(&mut inner.rng);
    }

    fn spawn<F: Future<Output = ()> + 'static>(&self, future: F) -> SchedulerHandle {
        self.scheduler
            .insert(FutureOperation::Background(future.boxed_local()))
    }
}
