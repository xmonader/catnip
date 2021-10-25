# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.

#===============================================================================

export CARGO_FLAGS ?= --release

#===============================================================================

all:
	cargo build $(CARGO_FLAGS)

clean:
	cargo clean && \
	rm Cargo.lock
