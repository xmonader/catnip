// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
use crate::protocols::{arp::ArpConfig, ethernet2::MacAddress, tcp, udp};
use arrayvec::ArrayVec;
use catwalk::{Scheduler, SchedulerHandle};
use rand::distributions::{Distribution, Standard};
use runtime::types::dmtr_sgarray_t;
use runtime::RuntimeBuf;
use std::{
    future::Future,
    net::Ipv4Addr,
    time::{Duration, Instant},
};

pub const RECEIVE_BATCH_SIZE: usize = 4;

pub trait PacketBuf<T>: Sized {
    fn header_size(&self) -> usize;
    fn write_header(&self, buf: &mut [u8]);
    fn body_size(&self) -> usize;
    fn take_body(self) -> Option<T>;
}

pub trait MemoryRuntime {
    type Buf: RuntimeBuf;

    #[allow(clippy::wrong_self_convention)]
    fn into_sgarray(&self, buf: Self::Buf) -> dmtr_sgarray_t;
    fn alloc_sgarray(&self, size: usize) -> dmtr_sgarray_t;
    fn free_sgarray(&self, sga: dmtr_sgarray_t);
    fn clone_sgarray(&self, sga: &dmtr_sgarray_t) -> Self::Buf;
}

pub trait SchedulerRuntime {
    type WaitFuture: Future<Output = ()>;

    fn wait(&self, duration: Duration) -> Self::WaitFuture;
    fn wait_until(&self, when: Instant) -> Self::WaitFuture;
    fn now(&self) -> Instant;
    fn advance_clock(&self, now: Instant);

    fn spawn<F: Future<Output = ()> + 'static>(&self, future: F) -> SchedulerHandle;
}

pub trait CommunicationRuntime: MemoryRuntime {
    fn transmit(&self, pkt: impl PacketBuf<Self::Buf>);
    fn receive(&self) -> ArrayVec<Self::Buf, RECEIVE_BATCH_SIZE>;
}

/// Common interface that transport layers should implement? E.g. DPDK and RDMA.
pub trait Runtime:
    Clone + Unpin + CommunicationRuntime + SchedulerRuntime + MemoryRuntime + 'static
{
    fn local_link_addr(&self) -> MacAddress;
    fn local_ipv4_addr(&self) -> Ipv4Addr;
    fn arp_options(&self) -> ArpConfig;
    fn tcp_options(&self) -> tcp::Options<Self>;
    fn udp_options(&self) -> udp::UdpConfig;

    fn rng_gen<T>(&self) -> T
    where
        Standard: Distribution<T>;
    fn rng_shuffle<T>(&self, slice: &mut [T]);

    fn scheduler(&self) -> &Scheduler;
}
