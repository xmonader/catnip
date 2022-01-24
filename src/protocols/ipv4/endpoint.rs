// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::protocols::ip;
use std::net::Ipv4Addr;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ipv4Endpoint {
    addr: Ipv4Addr,
    port: ip::Port,
}

/// Associate functions.
impl Ipv4Endpoint {
    /// Constructs a new [Ipv4Endpoint].
    pub fn new(addr: Ipv4Addr, port: ip::Port) -> Ipv4Endpoint {
        Ipv4Endpoint { addr, port }
    }

    /// Returns the [Ipv4Addr] associated to the target [Ipv4Endpoint].
    pub fn get_address(&self) -> Ipv4Addr {
        self.addr
    }

    /// Returns the [ip::Port] associated to the target [Ipv4Endpoint].
    pub fn get_port(&self) -> ip::Port {
        self.port
    }
}
