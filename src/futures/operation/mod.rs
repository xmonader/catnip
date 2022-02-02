// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

pub mod udp;

pub use self::udp::UdpOperation;

use crate::{protocols::tcp::operations::TcpOperation, runtime::Runtime};
use futures::Future;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

//==============================================================================
// Structures
//==============================================================================

/// The different types of operations our [Scheduler] can hold and multiplex between.
///
/// [Operation]s are tasks (top-level futures which are managed by our scheduler). This is
/// the granularity of our scheduling (our schedulable units).
///
/// Most operations are stored by our scheduler on a preallocated [PinSlab](unicycle::pin_slab::PinSlab)
/// to avoid expensive allocation, these represent shorter-lived work.
///
/// [Background](Operation::Background) tasks are heap-allocated as they are expected to live
/// long so we allocate them on the heap.
pub enum FutureOperation<RT: Runtime> {
    Tcp(TcpOperation<RT>),
    Udp(UdpOperation<RT>),

    // These are expected to have long lifetimes and be large enough to justify another allocation.
    Background(Pin<Box<dyn Future<Output = ()>>>),
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Simple wrapper which calls the corresponding [poll](Future::poll) method for each enum variant's
/// type.
impl<RT: Runtime> Future for FutureOperation<RT> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        match self.get_mut() {
            FutureOperation::Tcp(ref mut f) => Future::poll(Pin::new(f), ctx),
            FutureOperation::Udp(ref mut f) => Future::poll(Pin::new(f), ctx),
            FutureOperation::Background(ref mut f) => Future::poll(Pin::new(f), ctx),
        }
    }
}

impl<T, RT> From<T> for FutureOperation<RT>
where
    RT: Runtime,
    T: Into<TcpOperation<RT>>,
{
    fn from(f: T) -> Self {
        FutureOperation::Tcp(f.into())
    }
}
