// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod background;
mod ctrlblk;
mod sender;

pub use self::ctrlblk::ControlBlock;
pub use self::ctrlblk::State;
pub use self::sender::congestion_ctrl as cc;

use self::background::background;
use crate::protocols::ipv4::Ipv4Endpoint;
use crate::{protocols::tcp::segment::TcpHeader, runtime::Runtime};
use catwalk::SchedulerHandle;
use futures::channel::mpsc;
use runtime::fail::Fail;
use runtime::queue::IoQueueDescriptor;
use std::{
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

pub struct EstablishedSocket<RT: Runtime> {
    pub cb: Rc<ControlBlock<RT>>,
    #[allow(unused)]
    background_work: SchedulerHandle,
}

impl<RT: Runtime> EstablishedSocket<RT> {
    pub fn new(
        cb: ControlBlock<RT>,
        fd: IoQueueDescriptor,
        dead_socket_tx: mpsc::UnboundedSender<IoQueueDescriptor>,
    ) -> Self {
        let cb = Rc::new(cb);
        let future = background(cb.clone(), fd, dead_socket_tx);
        let handle = cb.rt().spawn(future);
        Self {
            cb: cb.clone(),
            background_work: handle,
        }
    }

    pub fn receive(&self, header: &TcpHeader, data: RT::Buf) {
        self.cb.receive(header, data)
    }

    pub fn send(&self, buf: RT::Buf) -> Result<(), Fail> {
        self.cb.send(buf)
    }

    pub fn poll_recv(&self, ctx: &mut Context) -> Poll<Result<RT::Buf, Fail>> {
        self.cb.poll_recv(ctx)
    }

    pub fn close(&self) -> Result<(), Fail> {
        self.cb.close()
    }

    pub fn remote_mss(&self) -> usize {
        self.cb.remote_mss()
    }

    pub fn current_rto(&self) -> Duration {
        self.cb.rto_current()
    }

    pub fn endpoints(&self) -> (Ipv4Endpoint, Ipv4Endpoint) {
        (self.cb.get_local(), self.cb.get_remote())
    }
}
