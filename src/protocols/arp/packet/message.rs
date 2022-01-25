// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::ArpHeader;
use crate::{protocols::ethernet2::Ethernet2Header, runtime::PacketBuf};
use std::marker::PhantomData;

//==============================================================================
// Structures
//==============================================================================

#[derive(Clone, Debug)]
pub struct ArpMessage<T> {
    ethernet2_hdr: Ethernet2Header,
    header: ArpHeader,
    _body_marker: PhantomData<T>,
}

//==============================================================================
// Associate Functions
//==============================================================================

impl<T> ArpMessage<T> {
    /// Creates an ARP message.
    pub fn new(header: Ethernet2Header, pdu: ArpHeader) -> Self {
        Self {
            ethernet2_hdr: header,
            header: pdu,
            _body_marker: PhantomData,
        }
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

impl<T> PacketBuf<T> for ArpMessage<T> {
    fn header_size(&self) -> usize {
        self.ethernet2_hdr.compute_size() + self.header.compute_size()
    }

    fn body_size(&self) -> usize {
        0
    }

    fn write_header(&self, buf: &mut [u8]) {
        let eth_hdr_size = self.ethernet2_hdr.compute_size();
        let arp_pdu_size = self.header.compute_size();
        let mut cur_pos = 0;

        self.ethernet2_hdr
            .serialize(&mut buf[cur_pos..(cur_pos + eth_hdr_size)]);
        cur_pos += eth_hdr_size;

        self.header
            .serialize(&mut buf[cur_pos..(cur_pos + arp_pdu_size)]);
    }

    fn take_body(self) -> Option<T> {
        None
    }
}
