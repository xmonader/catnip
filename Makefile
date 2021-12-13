# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.

#===============================================================================

export CARGO ?= $(HOME)/.cargo/bin/cargo

export BUILD ?=

#===============================================================================

all:
	$(CARGO) build --all $(BUILD) $(CARGO_FLAGS)

test:
	RUST_LOG=trace $(CARGO) test $(BUILD) $(CARGO_FLAGS) $(TEST) -- --nocapture

clean:
	rm -rf target && \
	$(CARGO) clean && \
	rm -f Cargo.lock
