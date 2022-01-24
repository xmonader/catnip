// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod config;
pub mod datagram;
mod listener;
mod operations;
pub mod peer;
mod socket;

#[cfg(test)]
mod tests;

pub use config::UdpConfig;
pub use datagram::UdpHeader;
pub use operations::PopFuture as UdpPopFuture;
pub use operations::UdpOperation;
pub use peer::UdpPeer as Peer;
