// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::{cell::UnsafeCell, rc::Rc, task::Waker};

struct WakerSlot(UnsafeCell<Option<Waker>>);

unsafe impl Send for WakerSlot {}
unsafe impl Sync for WakerSlot {}

pub struct SharedWaker(Rc<WakerSlot>);

impl Clone for SharedWaker {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Default for SharedWaker {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedWaker {
    pub fn new() -> Self {
        Self(Rc::new(WakerSlot(UnsafeCell::new(None))))
    }

    pub fn wake(&self) {
        let s = unsafe {
            let waker = &self.0;
            let cell = &waker.0;
            &mut *cell.get()
        };
        if let Some(waker) = s.take() {
            waker.wake();
        }
    }
}
