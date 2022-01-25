// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail, protocols::ethernet2::EtherType2, protocols::ethernet2::MacAddress,
    runtime::RuntimeBuf,
};
use byteorder::{ByteOrder, NetworkEndian};
use std::convert::{TryFrom, TryInto};

const ETHERNET2_HEADER_SIZE: usize = 14;

#[derive(Clone, Debug)]
pub struct Ethernet2Header {
    // Bytes 0..6
    dst_addr: MacAddress,
    // Bytes 6..12
    src_addr: MacAddress,
    // Bytes 12..14
    ether_type: EtherType2,
}

impl Ethernet2Header {
    /// Creates a header for an Ethernet frame.
    pub fn new(dst_addr: MacAddress, src_addr: MacAddress, ether_type: EtherType2) -> Self {
        Self {
            dst_addr,
            src_addr,
            ether_type,
        }
    }

    pub fn compute_size(&self) -> usize {
        ETHERNET2_HEADER_SIZE
    }

    pub fn parse<T: RuntimeBuf>(mut buf: T) -> Result<(Self, T), Fail> {
        if buf.len() < ETHERNET2_HEADER_SIZE {
            return Err(Fail::Malformed {
                details: "Frame too small",
            });
        }
        let hdr_buf = &buf[..ETHERNET2_HEADER_SIZE];
        let dst_addr = MacAddress::from_bytes(&hdr_buf[0..6]);
        let src_addr = MacAddress::from_bytes(&hdr_buf[6..12]);
        let ether_type = EtherType2::try_from(NetworkEndian::read_u16(&hdr_buf[12..14]))?;
        let hdr = Self {
            dst_addr,
            src_addr,
            ether_type,
        };

        buf.adjust(ETHERNET2_HEADER_SIZE);
        Ok((hdr, buf))
    }

    pub fn serialize(&self, buf: &mut [u8]) {
        let buf: &mut [u8; ETHERNET2_HEADER_SIZE] = buf.try_into().unwrap();
        buf[0..6].copy_from_slice(&self.dst_addr.octets());
        buf[6..12].copy_from_slice(&self.src_addr.octets());
        NetworkEndian::write_u16(&mut buf[12..14], self.ether_type as u16);
    }

    pub fn dst_addr(&self) -> MacAddress {
        self.dst_addr
    }

    pub fn src_addr(&self) -> MacAddress {
        self.src_addr
    }

    pub fn ether_type(&self) -> EtherType2 {
        self.ether_type
    }
}
