// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::fail::Fail;
use num_traits::FromPrimitive;
use std::convert::TryFrom;

#[repr(u8)]
#[derive(FromPrimitive, Copy, Clone, PartialEq, Eq, Debug)]
/// Ipv4 Protocol
pub enum Ipv4Protocol2 {
    Icmpv4 = 0x01,
    Tcp = 0x06,
    Udp = 0x11,
}

/// TryFrom trait implementation.
impl TryFrom<u8> for Ipv4Protocol2 {
    type Error = Fail;

    fn try_from(n: u8) -> Result<Self, Fail> {
        match FromPrimitive::from_u8(n) {
            Some(n) => Ok(n),
            None => Err(Fail::Unsupported {
                details: "Unsupported IPv4 protocol",
            }),
        }
    }
}
