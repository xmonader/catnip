// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::protocols::ipv4::Endpoint;

//==============================================================================
// Constants & Structures
//==============================================================================

/// UDP Socket
#[derive(Debug)]
pub struct UdpSocket {
    /// Local endpoint.
    local: Option<Endpoint>,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions.
impl UdpSocket {
    /// Gets the local endpoint of the target [UdpSocket].
    pub fn get_local(&self) -> Option<Endpoint> {
        self.local
    }

    /// Sets the local endpoint of the target [UdpSocket].
    pub fn set_local(&mut self, local: Option<Endpoint>) {
        self.local = local;
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Default trait implementation.
impl Default for UdpSocket {
    /// Creates a [UdpSocket] with default values.
    fn default() -> Self {
        Self { local: None }
    }
}
