// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
#![cfg_attr(feature = "strict", deny(clippy:all))]
#![deny(clippy::all)]
#![feature(new_uninit)]
#![feature(never_type)]
#![feature(try_blocks)]
#![feature(test)]
#![feature(min_type_alias_impl_trait)]
#![recursion_limit = "512"]

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate log;

#[macro_use]
extern crate derive_more;

#[cfg(test)]
pub mod test_helpers;

pub mod collections;
pub mod futures;
pub mod interop;
pub mod libos;
pub mod logging;
pub mod operations;
pub mod options;
pub mod protocols;
pub mod runtime;
pub mod timer;
