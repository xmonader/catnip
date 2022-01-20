// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::{cell::UnsafeCell, mem};

pub struct WakerU64(UnsafeCell<u64>);

unsafe impl Sync for WakerU64 {}

impl WakerU64 {
    pub fn new(val: u64) -> Self {
        WakerU64(UnsafeCell::new(val))
    }

    pub fn fetch_or(&self, val: u64) {
        let s = unsafe { &mut *self.0.get() };
        *s |= val;
    }

    pub fn fetch_and(&self, val: u64) {
        let s = unsafe { &mut *self.0.get() };
        *s &= val;
    }

    pub fn fetch_add(&self, val: u64) -> u64 {
        let s = unsafe { &mut *self.0.get() };
        let old = *s;
        *s += val;
        old
    }

    pub fn fetch_sub(&self, val: u64) -> u64 {
        let s = unsafe { &mut *self.0.get() };
        let old = *s;
        *s -= val;
        old
    }

    pub fn load(&self) -> u64 {
        let s = unsafe { &mut *self.0.get() };
        *s
    }

    pub fn swap(&self, val: u64) -> u64 {
        let s = unsafe { &mut *self.0.get() };
        mem::replace(s, val)
    }
}
