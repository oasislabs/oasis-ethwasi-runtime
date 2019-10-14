#!/usr/bin/env gmake

# Oasis Core binary base path.
OASIS_CORE_ROOT_PATH ?= .oasis-core

# Runtime binary base path.
RUNTIME_ROOT_PATH ?= .runtime

# Ekiden cargo target directory.
OASIS_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),$(OASIS_CORE_ROOT_PATH)/target)

# Runtime cargo target directory.
RUNTIME_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)

# Genesis files.
GENESIS_ROOT_PATH ?= resources/genesis
GENESIS_FILES ?= \
	genesis.json \
	genesis_testing.json

# Extra build args.
EXTRA_BUILD_ARGS := $(if $(RELEASE),--release,)

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
	runtime gateway genesis \
	genesis-update \
	clean clean-test-e2e \
	fmt \
	run-gateway run-gateway-sgx \
	test test-unit test-e2e

all: check runtime gateway
	@$(ECHO) "$(CYAN)*** Everything built successfully!$(OFF)"

check: check-tools check-oasis-core

check-tools:
	@which cargo-elf2sgxs >/dev/null || ( \
		$(ECHO) "$(RED)error:$(OFF) oasis-core-tools not installed (or not in PATH)" && \
		exit 1 \
	)

check-oasis-core:
	@test -x $(OASIS_CORE_ROOT_PATH)/go/oasis-node/oasis-node || ( \
		$(ECHO) "$(RED)error:$(OFF) oasis-node not found in $(OASIS_CORE_ROOT_PATH) (check OASIS_CORE_ROOT_PATH)" && \
		$(ECHO) "       Maybe you need to run \"make symlink-artifacts\"?" && \
		exit 1 \
	)

symlink-artifacts:
	@$(ECHO) "$(CYAN)*** Symlinking Oasis Core and runtime build artifacts...$(OFF)"
	@export OASIS_CARGO_TARGET_DIR=$(OASIS_CARGO_TARGET_DIR) && \
		scripts/symlink_artifacts.sh "$(OASIS_CORE_ROOT_PATH)" "$(OASIS_CORE_SRC_PATH)" "$(RUNTIME_ROOT_PATH)" $$(pwd)
	@$(ECHO) "$(CYAN)*** Symlinking done!$(OFF)"

runtime: check-oasis-core
	@$(ECHO) "$(CYAN)*** Building oasis-runtime...$(OFF)"
	@cargo build -p oasis-runtime $(EXTRA_BUILD_ARGS) --target x86_64-fortanix-unknown-sgx
	@cargo build -p oasis-runtime $(EXTRA_BUILD_ARGS)
	@cargo elf2sgxs $(EXTRA_BUILD_ARGS)

gateway:
	@$(ECHO) "$(CYAN)*** Building web3-gateway...$(OFF)"
	@cargo build -p web3-gateway $(EXTRA_BUILD_ARGS)

genesis:
	@$(ECHO) "$(CYAN)*** Building genesis utilities...$(OFF)"
	@cargo build -p genesis $(EXTRA_BUILD_ARGS)

genesis-update:
	@$(ECHO) "$(CYAN)*** Generating Ekiden-compatible genesis files...$(OFF)"
	@for g in $(GENESIS_FILES); do \
		$(ECHO) "$(MAGENTA)  * Genesis file: $$g$(OFF)"; \
		cargo run -p genesis $(EXTRA_BUILD_ARGS) --bin genesis-init -- \
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
#	@$(ECHO) "$(CYAN)*** Starting Ekiden node and Web3 gateway (SGX)...$(OFF)"
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
