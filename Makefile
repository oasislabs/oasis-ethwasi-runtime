#!/usr/bin/env gmake

# Oasis Core binary base path.
OASIS_CORE_ROOT_PATH ?= .oasis-core

# Runtime binary base path.
RUNTIME_ROOT_PATH ?= .runtime

# oasis-core cargo target directory.
OASIS_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),$(OASIS_CORE_ROOT_PATH)/target)

# Runtime cargo target directory.
RUNTIME_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),$(shell pwd)/target)/default
RUNTIME_SGX_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),$(shell pwd)target)/sgx

# List of runtime paths to build.
RUNTIMES := . \
	keymanager-runtime

# Genesis files.
GENESIS_ROOT_PATH ?= resources/genesis
GENESIS_FILES ?= \
	genesis.json \
	genesis_testing.json

# Extra build args.
EXTRA_BUILD_ARGS := $(if $(RELEASE),--release,)

# Extra args specifically for cargo build.
CARGO_BUILD_ARGS := $(if $(BENCHMARKS),--features benchmarking,)

# Check if we're running in an interactive terminal.
ISATTY := $(shell [ -t 0 ] && echo 1)

ifdef ISATTY
# Running in interactive terminal, OK to use colors!
MAGENTA = \e[35;1m
CYAN = \e[36;1m
RED = \e[31;1m
OFF = \e[0m

# Built-in echo doesn't support '-e'.
ECHO = /bin/echo -e
else
# Don't use colors if not running interactively.
MAGENTA = ""
CYAN = ""
RED = ""
OFF = ""

# OK to use built-in echo.
ECHO = echo
endif


.PHONY: \
	all \
	check check-tools check-oasis-core \
	symlink-artifacts \
	build-runtimes gateway genesis \
	genesis-update \
	clean clean-test-e2e \
	fmt \
	run-gateway run-gateway-sgx \
	test test-unit test-e2e

all: check build-runtimes gateway genesis
	@$(ECHO) "$(CYAN)*** Everything built successfully!$(OFF)"

check: check-tools check-oasis-core

check-tools:
	@which cargo-elf2sgxs >/dev/null || ( \
		$(ECHO) "$(RED)error:$(OFF) oasis-core-tools not installed (or not in PATH)" && \
		exit 1 \
	)

check-oasis-core:
	@export OASIS_CORE_ROOT_PATH=$(OASIS_CORE_ROOT_PATH) && \
		scripts/check_artifacts.sh

symlink-artifacts:
	@$(ECHO) "$(CYAN)*** Symlinking Oasis Core and runtime build artifacts...$(OFF)"
	@export OASIS_CARGO_TARGET_DIR=$(OASIS_CARGO_TARGET_DIR) && \
		scripts/symlink_artifacts.sh "$(OASIS_CORE_ROOT_PATH)" "$(OASIS_CORE_SRC_PATH)" "$(RUNTIME_ROOT_PATH)" $$(pwd)
	@$(ECHO) "$(CYAN)*** Symlinking done!$(OFF)"

build-runtimes:
	@CARGO_TARGET_ROOT=$(shell pwd)/target && for e in $(RUNTIMES); do \
		$(ECHO) "$(MAGENTA)*** Building runtime: $$e...$(OFF)"; \
		(cd $$e && \
			CARGO_TARGET_DIR=$(RUNTIME_SGX_CARGO_TARGET_DIR) cargo build $(EXTRA_BUILD_ARGS) $(CARGO_BUILD_ARGS) --target x86_64-fortanix-unknown-sgx && \
			CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) cargo build $(EXTRA_BUILD_ARGS) $(CARGO_BUILD_ARGS) && \
			CARGO_TARGET_DIR=$(RUNTIME_SGX_CARGO_TARGET_DIR) cargo elf2sgxs $(EXTRA_BUILD_ARGS) \
		) || exit 1; \
	done

gateway:
	@$(ECHO) "$(CYAN)*** Building web3-gateway...$(OFF)"
	@CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) cargo build -p web3-gateway $(EXTRA_BUILD_ARGS) $(CARGO_BUILD_ARGS)

genesis:
	@$(ECHO) "$(CYAN)*** Building genesis utilities...$(OFF)"
	@CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) cargo build -p genesis $(EXTRA_BUILD_ARGS) $(CARGO_BUILD_ARGS)

genesis-update: genesis
	@$(ECHO) "$(CYAN)*** Generating oasis-core-compatible genesis files...$(OFF)"
	@for g in $(GENESIS_FILES); do \
		$(ECHO) "$(MAGENTA)  * Genesis file: $$g$(OFF)"; \
		CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) cargo run -p genesis $(EXTRA_BUILD_ARGS) --bin genesis-init -- \
			"$(GENESIS_ROOT_PATH)/$${g}" \
			"$(GENESIS_ROOT_PATH)/oasis_$${g}"; \
	done

benchmark: genesis
	@$(ECHO) "$(CYAN)*** Building benchmark client...$(OFF)"
	@make -C benchmark

run-gateway:
	@$(ECHO) "$(CYAN)*** Starting Oasis Network Runner and Web3 gateway...$(OFF)"
	@export OASIS_CORE_ROOT_PATH=$(OASIS_CORE_ROOT_PATH) RUNTIME_CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) GENESIS_ROOT_PATH=$(GENESIS_ROOT_PATH) && \
		scripts/gateway.sh 2>&1 | python scripts/color-log.py

# TODO: update gateway.sh to support SGX
#run-gateway-sgx:
#	@$(ECHO) "$(CYAN)*** Starting oasis-core node and Web3 gateway (SGX)...$(OFF)"
#	@export OASIS_CORE_ROOT_PATH=$(OASIS_CORE_ROOT_PATH) RUNTIME_CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) && \
#		scripts/gateway.sh single_node_sgx 2>&1 | python scripts/color-log.py

test: test-unit test-e2e

test-unit: check-oasis-core
	@$(ECHO) "$(CYAN)*** Running unit tests...$(OFF)"
	@cargo test \
		--features test \
		-p oasis-runtime-common \
		-p oasis-runtime \
		-p web3-gateway
	@make -C benchmark test

test-e2e: check-oasis-core
	@$(ECHO) "$(CYAN)*** Running E2E tests...$(OFF)"
	@export OASIS_CORE_ROOT_PATH=$(OASIS_CORE_ROOT_PATH) && \
		.buildkite/scripts/test_e2e.sh

fmt:
	@cargo fmt

clean-test-e2e:
	@$(ECHO) "$(CYAN)*** Cleaning up E2E tests...$(OFF)"
	@rm -rf .e2e
	@rm -rf tests/rpc-tests
	@rm -rf tests/e2e-tests

clean: clean-test-e2e
	@$(ECHO) "$(CYAN)*** Cleaning up...$(OFF)"
	@cargo clean
