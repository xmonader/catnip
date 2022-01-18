// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use slab::Slab;

//==============================================================================
// Constants & Structures
//==============================================================================

/// File Descriptor
pub type FileDescriptor = u32;

/// File Table Data
struct Inner {
    table: Slab<File>,
}

/// File Table
pub struct FileTable {
    inner: Inner,
}

/// File Types
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum File {
    TcpSocket,
    UdpSocket,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [FileTable].
impl FileTable {
    /// Creates a file table.
    pub fn new() -> Self {
        let inner = Inner { table: Slab::new() };
        Self { inner }
    }

    /// Allocates a new entry in the target file descriptor table.
    pub fn alloc(&mut self, file: File) -> FileDescriptor {
        let ix = self.inner.table.insert(file);
        ix as FileDescriptor
    }

    /// Gets the file associated with a file descriptor.
    pub fn get(&self, fd: FileDescriptor) -> Option<File> {
        if !self.inner.table.contains(fd as usize) {
            return None;
        }

        self.inner.table.get(fd as usize).cloned()
    }

    /// Releases an entry in the target file descriptor table.
    pub fn free(&mut self, fd: FileDescriptor) -> Option<File> {
        if !self.inner.table.contains(fd as usize) {
            return None;
        }

        let file = self.inner.table.remove(fd as usize);

        Some(file)
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Default trait implementation for [FileTable].
impl Default for FileTable {
    fn default() -> Self {
        Self::new()
    }
}
