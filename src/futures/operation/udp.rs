// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail, futures::result::FutureResult, operations::OperationResult,
    protocols::udp::UdpPopFuture, queue::IoQueueDescriptor, runtime::Runtime,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

//==============================================================================
// Constants & Structures
//==============================================================================

/// Operations on UDP Layer
pub enum UdpOperation<RT: Runtime> {
    Connect(IoQueueDescriptor, Result<(), Fail>),
    Push(IoQueueDescriptor, Result<(), Fail>),
    Pop(FutureResult<UdpPopFuture<RT>>),
}

//==============================================================================
// Associate Functions
//==============================================================================

impl<RT: Runtime> UdpOperation<RT> {
    pub fn expect_result(self) -> (IoQueueDescriptor, OperationResult<RT>) {
        match self {
            UdpOperation::Push(fd, Err(e)) | UdpOperation::Connect(fd, Err(e)) => {
                (fd, OperationResult::Failed(e))
            }
            UdpOperation::Connect(fd, Ok(())) => (fd, OperationResult::Connect),
            UdpOperation::Push(fd, Ok(())) => (fd, OperationResult::Push),

            UdpOperation::Pop(FutureResult {
                future,
                done: Some(Ok((addr, bytes))),
            }) => (future.get_qd(), OperationResult::Pop(addr, bytes)),
            UdpOperation::Pop(FutureResult {
                future,
                done: Some(Err(e)),
            }) => (future.get_qd(), OperationResult::Failed(e)),

            _ => panic!("Future not ready"),
        }
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Future trait implementation for [UdpOperation]
impl<RT: Runtime> Future for UdpOperation<RT> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<()> {
        match self.get_mut() {
            UdpOperation::Connect(..) | UdpOperation::Push(..) => Poll::Ready(()),
            UdpOperation::Pop(ref mut f) => Future::poll(Pin::new(f), ctx),
        }
    }
}
