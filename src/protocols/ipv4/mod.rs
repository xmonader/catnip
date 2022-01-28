// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod datagram;
mod endpoint;
mod protocol;

#[cfg(test)]
mod tests;

pub use datagram::Ipv4Header;
pub use endpoint::Ipv4Endpoint;
pub use protocol::Ipv4Protocol2;

pub use datagram::IPV4_HEADER_SIZE;
