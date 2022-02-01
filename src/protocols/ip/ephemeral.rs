// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{protocols::ip::Port, runtime::Runtime};
use runtime::fail::Fail;
use std::num::NonZeroU16;

//==============================================================================
// Constants
//==============================================================================

const FIRST_PRIVATE_PORT: u16 = 49152;
const LAST_PRIVATE_PORT: u16 = 65535;

//==============================================================================
// Structures
//==============================================================================

pub struct EphemeralPorts {
    ports: Vec<Port>,
}

//==============================================================================
// Associate Functions
//==============================================================================

impl EphemeralPorts {
    pub fn new<RT: Runtime>(rt: &RT) -> Self {
        let mut ports: Vec<Port> = Vec::<Port>::new();
        for n in FIRST_PRIVATE_PORT..LAST_PRIVATE_PORT {
            let p: Port = Port::new(NonZeroU16::new(n).expect("failed to allocate ephemeral port"));
            ports.push(p);
        }

        rt.rng_shuffle(&mut ports[..]);
        Self { ports }
    }

    pub fn alloc(&mut self) -> Result<Port, Fail> {
        self.ports.pop().ok_or(Fail::ResourceExhausted {
            details: "Out of private ports",
        })
    }

    pub fn free(&mut self, port: Port) {
        self.ports.push(port);
    }
}
