// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::fail::Fail;
use num_traits::FromPrimitive;
use std::convert::TryFrom;

#[repr(u16)]
#[derive(FromPrimitive, Copy, Clone, PartialEq, Eq, Debug)]
pub enum EtherType2 {
    Arp = 0x806,
    Ipv4 = 0x800,
}

impl TryFrom<u16> for EtherType2 {
    type Error = Fail;

    fn try_from(n: u16) -> Result<Self, Fail> {
        match FromPrimitive::from_u16(n) {
            Some(n) => Ok(n),
            None => Err(Fail::Unsupported {
                details: "Unsupported ETHERTYPE",
            }),
        }
    }
}
