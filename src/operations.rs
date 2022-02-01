// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{protocols::ipv4::Ipv4Endpoint, runtime::Runtime};
use runtime::fail::Fail;
use runtime::queue::IoQueueDescriptor;
use std::fmt;

//==============================================================================
// Structures
//==============================================================================

pub enum OperationResult<RT: Runtime> {
    Connect,
    Accept(IoQueueDescriptor),
    Push,
    Pop(Option<Ipv4Endpoint>, RT::Buf),
    Failed(Fail),
}

//==============================================================================
// Trait Implementations
//==============================================================================

impl<RT: Runtime> fmt::Debug for OperationResult<RT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OperationResult::Connect => write!(f, "Connect"),
            OperationResult::Accept(..) => write!(f, "Accept"),
            OperationResult::Push => write!(f, "Push"),
            OperationResult::Pop(..) => write!(f, "Pop"),
            OperationResult::Failed(ref e) => write!(f, "Failed({:?})", e),
        }
    }
}
