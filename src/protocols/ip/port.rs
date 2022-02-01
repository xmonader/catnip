// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use runtime::fail::Fail;
use std::{convert::TryFrom, num::NonZeroU16};

//==============================================================================
// Constants
//==============================================================================

const FIRST_PRIVATE_PORT: u16 = 49152;

//==============================================================================
// Structures
//==============================================================================

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Display, Ord, PartialOrd)]
pub struct Port(NonZeroU16);

//==============================================================================
// Trait Implementations
//==============================================================================

impl From<Port> for u16 {
    fn from(val: Port) -> Self {
        u16::from(val.0)
    }
}

impl TryFrom<u16> for Port {
    type Error = Fail;

    fn try_from(n: u16) -> Result<Self, Fail> {
        Ok(Port(NonZeroU16::new(n).ok_or(Fail::OutOfRange {
            details: "port number may not be zero",
        })?))
    }
}

//==============================================================================
// Associate Functions
//==============================================================================

impl Port {
    pub fn first_private_port() -> Port {
        Port::try_from(FIRST_PRIVATE_PORT).unwrap()
    }

    pub fn is_private(self) -> bool {
        self.0.get() >= FIRST_PRIVATE_PORT
    }

    pub(crate) fn new(num: NonZeroU16) -> Self {
        Self { 0: num }
    }
}
