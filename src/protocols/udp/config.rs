// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//==============================================================================
// Constants & Structures
//==============================================================================

/// Control Options for UDP
#[derive(Clone, Debug)]
pub struct UdpConfig {
    /// Enable checksum offload on receiver side?
    rx_checksum: bool,
    /// Enable checksum offload on sender side?
    tx_checksum: bool,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [UdpConfig].
impl UdpConfig {
    /// Creates custom options for UDP.
    pub fn new(rx_checksum: bool, tx_checksum: bool) -> Self {
        Self {
            rx_checksum,
            tx_checksum,
        }
    }

    /// Returns whether or not checksum offload on receiver side is enabled.
    pub fn get_rx_checksum(&self) -> bool {
        self.rx_checksum
    }

    /// Returns whether or not checksum offload on sender side is enabled.
    pub fn get_tx_checksum(&self) -> bool {
        self.tx_checksum
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Implementation of [Default] trait for [UdpConfig].
impl Default for UdpConfig {
    /// Creates default options for UDP.
    fn default() -> Self {
        UdpConfig {
            rx_checksum: false,
            tx_checksum: false,
        }
    }
}

//==============================================================================
// Unit Tests
//==============================================================================

#[cfg(test)]
mod tests {
    use super::UdpConfig;

    /// Tests instantiations flavors for [UdpConfig].
    #[test]
    fn test_udp_options() {
        //Default options.
        let options_default = UdpConfig::default();
        assert!(!options_default.get_rx_checksum());
        assert!(!options_default.get_tx_checksum());

        // Custom options.
        let options_custom = UdpConfig::new(true, true);
        assert!(options_custom.get_rx_checksum());
        assert!(options_custom.get_tx_checksum());
    }
}
