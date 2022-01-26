// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::protocols::ethernet2::MacAddress;
use std::collections::HashMap;
use std::{net::Ipv4Addr, time::Duration};

//==============================================================================
// Structures
//==============================================================================

#[derive(Clone, Debug)]
pub struct ArpConfig {
    cache_ttl: Duration,
    request_timeout: Duration,
    retry_count: usize,

    initial_values: HashMap<Ipv4Addr, MacAddress>,
    disable_arp: bool,
}

//==============================================================================
// Trait Implementations
//==============================================================================

impl Default for ArpConfig {
    fn default() -> Self {
        ArpConfig {
            cache_ttl: Duration::from_secs(15),
            request_timeout: Duration::from_secs(20),
            retry_count: 5,
            initial_values: HashMap::new(),
            disable_arp: false,
        }
    }
}

//==============================================================================
// Associate Functions
//==============================================================================

impl ArpConfig {
    pub fn new(
        cache_ttl: Duration,
        request_timeout: Duration,
        retry_count: usize,
        initial_values: HashMap<Ipv4Addr, MacAddress>,
        disable_arp: bool,
    ) -> Self {
        ArpConfig {
            cache_ttl,
            request_timeout,
            retry_count,
            initial_values,
            disable_arp,
        }
    }

    pub fn get_cache_ttl(&self) -> Duration {
        self.cache_ttl
    }

    pub fn get_request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub fn get_retry_count(&self) -> usize {
        self.retry_count
    }

    pub fn get_disable_arp(&self) -> bool {
        self.disable_arp
    }

    pub fn get_initial_values(&self) -> &HashMap<Ipv4Addr, MacAddress> {
        &self.initial_values
    }
}
