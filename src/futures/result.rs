// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

//==============================================================================
// Structures
//==============================================================================

pub struct FutureResult<F: Future> {
    pub future: F,
    pub done: Option<F::Output>,
}

//==============================================================================
// Associate Functions
//==============================================================================

impl<F: Future> FutureResult<F> {
    pub fn new(future: F, done: Option<F::Output>) -> Self {
        Self { future, done }
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

impl<F: Future + Unpin> Future for FutureResult<F>
where
    F::Output: Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<()> {
        let self_ = self.get_mut();
        if self_.done.is_some() {
            panic!("Polled after completion")
        }
        let result = match Future::poll(Pin::new(&mut self_.future), ctx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(r) => r,
        };
        self_.done = Some(result);
        Poll::Ready(())
    }
}
