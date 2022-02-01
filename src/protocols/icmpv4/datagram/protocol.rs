// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use byteorder::{ByteOrder, NetworkEndian};
use runtime::fail::Fail;

//==============================================================================
// Icmpv4Type2
//==============================================================================

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Icmpv4Type2 {
    EchoReply { id: u16, seq_num: u16 },
    DestinationUnreachable,
    SourceQuench,
    RedirectMessage,
    EchoRequest { id: u16, seq_num: u16 },
    RouterAdvertisement,
    RouterSolicitation,
    TimeExceeded,
    BadIpHeader,
    Timestamp,
    TimestampReply,
}

impl Icmpv4Type2 {
    pub fn parse(type_byte: u8, rest_of_header: &[u8; 4]) -> Result<Self, Fail> {
        use Icmpv4Type2::*;
        match type_byte {
            0 => {
                let id = NetworkEndian::read_u16(&rest_of_header[0..2]);
                let seq_num = NetworkEndian::read_u16(&rest_of_header[2..4]);
                Ok(EchoReply { id, seq_num })
            }
            3 => Ok(DestinationUnreachable),
            4 => Ok(SourceQuench),
            5 => Ok(RedirectMessage),
            8 => {
                let id = NetworkEndian::read_u16(&rest_of_header[0..2]);
                let seq_num = NetworkEndian::read_u16(&rest_of_header[2..4]);
                Ok(EchoRequest { id, seq_num })
            }
            9 => Ok(RouterAdvertisement),
            10 => Ok(RouterSolicitation),
            11 => Ok(TimeExceeded),
            12 => Ok(BadIpHeader),
            13 => Ok(Timestamp),
            14 => Ok(TimestampReply),
            _ => Err(Fail::Malformed {
                details: "Invalid type byte",
            }),
        }
    }

    pub fn serialize(&self) -> (u8, [u8; 4]) {
        use Icmpv4Type2::*;
        let zero = [0u8; 4];
        match self {
            EchoReply { id, seq_num } => {
                let [id1, id2] = id.to_be_bytes();
                let [seq1, seq2] = seq_num.to_be_bytes();
                (0, [id1, id2, seq1, seq2])
            }
            DestinationUnreachable => (3, zero),
            SourceQuench => (4, zero),
            RedirectMessage => (5, zero),
            EchoRequest { id, seq_num } => {
                let [id1, id2] = id.to_be_bytes();
                let [seq1, seq2] = seq_num.to_be_bytes();
                (8, [id1, id2, seq1, seq2])
            }
            RouterAdvertisement => (9, zero),
            RouterSolicitation => (10, zero),
            TimeExceeded => (11, zero),
            BadIpHeader => (12, zero),
            Timestamp => (13, zero),
            TimestampReply => (14, zero),
        }
    }
}
