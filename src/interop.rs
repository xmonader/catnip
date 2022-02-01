// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{operations::OperationResult, runtime::Runtime};
use runtime::queue::IoQueueDescriptor;
use runtime::types::dmtr_accept_result_t;
use runtime::types::dmtr_opcode_t;
use runtime::types::dmtr_qr_value_t;
use runtime::types::dmtr_qresult_t;
use std::mem;

pub fn pack_result<RT: Runtime>(
    rt: &RT,
    result: OperationResult<RT>,
    qd: IoQueueDescriptor,
    qt: u64,
) -> dmtr_qresult_t {
    match result {
        OperationResult::Connect => dmtr_qresult_t {
            qr_opcode: dmtr_opcode_t::DMTR_OPC_CONNECT,
            qr_qd: qd.into(),
            qr_qt: qt,
            qr_value: unsafe { mem::zeroed() },
        },
        OperationResult::Accept(new_qd) => {
            let sin = unsafe { mem::zeroed() };
            let qr_value = dmtr_qr_value_t {
                ares: dmtr_accept_result_t {
                    qd: new_qd.into(),
                    addr: sin,
                },
            };
            dmtr_qresult_t {
                qr_opcode: dmtr_opcode_t::DMTR_OPC_ACCEPT,
                qr_qd: qd.into(),
                qr_qt: qt,
                qr_value,
            }
        }
        OperationResult::Push => dmtr_qresult_t {
            qr_opcode: dmtr_opcode_t::DMTR_OPC_PUSH,
            qr_qd: qd.into(),
            qr_qt: qt,
            qr_value: unsafe { mem::zeroed() },
        },
        OperationResult::Pop(addr, bytes) => {
            let mut sga = rt.into_sgarray(bytes);
            if let Some(addr) = addr {
                sga.sga_addr.sin_port = addr.get_port().into();
                sga.sga_addr.sin_addr.s_addr = u32::from_le_bytes(addr.get_address().octets());
            }
            let qr_value = dmtr_qr_value_t { sga };
            dmtr_qresult_t {
                qr_opcode: dmtr_opcode_t::DMTR_OPC_POP,
                qr_qd: qd.into(),
                qr_qt: qt,
                qr_value,
            }
        }
        OperationResult::Failed(e) => {
            warn!("Operation Failed: {:?}", e);
            dmtr_qresult_t {
                qr_opcode: dmtr_opcode_t::DMTR_OPC_FAILED,
                qr_qd: qd.into(),
                qr_qt: qt,
                qr_value: unsafe { mem::zeroed() },
            }
        }
    }
}
