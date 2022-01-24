// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::protocols::ipv4::Endpoint;
use std::{cell::RefCell, collections::VecDeque, rc::Rc, task::Waker};

//==============================================================================
// Listener
//==============================================================================

/// UDP Listener
struct Listener<T> {
    buf: VecDeque<(Option<Endpoint>, T)>,
    waker: Option<Waker>,
}

/// Associate functions.
impl<T> Listener<T> {
    /// Pushes data to the target [Listener].
    fn push_data(&mut self, endpoint: Option<Endpoint>, data: T) {
        self.buf.push_back((endpoint, data));
    }

    /// Pops data from the target [Listener].
    fn pop_data(&mut self) -> Option<(Option<Endpoint>, T)> {
        self.buf.pop_front()
    }

    /// Takes the waker value stored in the target [Listener].
    fn take_waker(&mut self) -> Option<Waker> {
        self.waker.take()
    }

    /// Places a waker in the target [Listener].
    fn put_waker(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }
}

/// Default trait implementation.
impl<T> Default for Listener<T> {
    /// Creates a [Listener] with the default values.
    fn default() -> Self {
        Self {
            buf: VecDeque::new(),
            waker: None,
        }
    }
}
//==============================================================================
// SharedListener
//==============================================================================

#[derive(Clone)]
pub struct SharedListener<T> {
    l: Rc<RefCell<Listener<T>>>,
}

/// Associate functions.
impl<T> SharedListener<T> {
    fn new(l: Listener<T>) -> Self {
        Self {
            l: Rc::new(RefCell::new(l)),
        }
    }

    /// Pushes data to the target [SharedListener].
    pub fn push_data(&self, endpoint: Option<Endpoint>, data: T) {
        self.l.borrow_mut().push_data(endpoint, data);
    }

    /// Pops data from the target [SharedListener].
    pub fn pop_data(&self) -> Option<(Option<Endpoint>, T)> {
        self.l.borrow_mut().pop_data()
    }

    /// Takes the waker value stored in the target [SharedListener].
    pub fn take_waker(&self) -> Option<Waker> {
        self.l.borrow_mut().take_waker()
    }

    /// Places a waker in the target [SharedListener].
    pub fn put_waker(&self, waker: Option<Waker>) {
        self.l.borrow_mut().put_waker(waker)
    }
}

/// Default trait implementation.
impl<T> Default for SharedListener<T> {
    /// Creates a [SharedListener] with the default values.
    fn default() -> Self {
        let l = Listener::<T>::default();
        SharedListener::new(l)
    }
}
