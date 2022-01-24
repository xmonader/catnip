// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::super::listener::SharedListener;
use crate::{
    fail::Fail, protocols::ipv4::Ipv4Endpoint, queue::IoQueueDescriptor, runtime::Runtime,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

//==============================================================================
// Constants & Structures
//==============================================================================

/// Future for Pop Operation
pub struct UdpPopFuture<RT: Runtime> {
    /// File descriptor.
    qd: IoQueueDescriptor,
    /// Listener.
    listener: Result<SharedListener<RT::Buf>, Fail>,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [PopFuture].
impl<RT: Runtime> UdpPopFuture<RT> {
    /// Creates a future for the pop operation.
    pub fn new(qd: IoQueueDescriptor, listener: Result<SharedListener<RT::Buf>, Fail>) -> Self {
        Self { qd, listener }
    }

    /// Returns the [IoQueueDescriptor] associated to the target [UdpPopFuture].
    pub fn get_qd(&self) -> IoQueueDescriptor {
        self.qd
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Future trait implementation for [PopFuture].
impl<RT: Runtime> Future for UdpPopFuture<RT> {
    type Output = Result<(Option<Ipv4Endpoint>, RT::Buf), Fail>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let self_ = self.get_mut();
        match self_.listener.clone() {
            Err(ref e) => Poll::Ready(Err(e.clone())),
            Ok(listener) => {
                if let Some(r) = listener.pop_data() {
                    return Poll::Ready(Ok(r));
                }
                let waker = ctx.waker();
                listener.put_waker(Some(waker.clone()));
                Poll::Pending
            }
        }
    }
}
