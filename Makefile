# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.

#===============================================================================

export CARGO ?= $(HOME)/.cargo/bin/cargo

export BUILD ?= --release

export CARGO_FLAGS ?= $(BUILD)

#===============================================================================

all:
	$(CARGO) build --all $(CARGO_FLAGS)

clean:
	$(CARGO) clean && \
	rm -f Cargo.lock
