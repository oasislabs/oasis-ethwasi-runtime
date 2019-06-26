#!/usr/bin/env gmake

# Ekiden binary base path.
EKIDEN_ROOT_PATH ?= .ekiden

# Runtime binary base path.
RUNTIME_ROOT_PATH ?= .runtime

# Ekiden cargo target directory.
EKIDEN_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),$(EKIDEN_ROOT_PATH)/target)

# Runtime cargo target directory.
RUNTIME_CARGO_TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)

# Key manager enclave path.
KM_ENCLAVE_PATH ?= $(EKIDEN_CARGO_TARGET_DIR)/x86_64-fortanix-unknown-sgx/debug/ekiden-keymanager-runtime.sgxs

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
	check check-tools check-ekiden \
	download-artifacts symlink-artifacts \
	runtime gateway genesis \
	genesis-update \
	clean clean-test-e2e \
	fmt \
	run-gateway run-gateway-sgx \
	test test-unit test-e2e

all: check runtime gateway
	@$(ECHO) "$(CYAN)*** Everything built successfully!$(OFF)"

check: check-tools check-ekiden

check-tools:
	@which cargo-elf2sgxs >/dev/null || ( \
		$(ECHO) "$(RED)error:$(OFF) ekiden-tools not installed (or not in PATH)" && \
		exit 1 \
	)

check-ekiden:
	@test -x $(EKIDEN_ROOT_PATH)/go/ekiden/ekiden || ( \
		$(ECHO) "$(RED)error:$(OFF) ekiden node not found in $(EKIDEN_ROOT_PATH) (check EKIDEN_ROOT_PATH)" && \
		$(ECHO) "       Maybe you need to run \"make symlink-artifacts\" or \"make download-artifacts\"?" && \
		exit 1 \
	)
	@test -f $(KM_ENCLAVE_PATH) || ( \
		$(ECHO) "$(RED)error:$(OFF) ekiden key manager enclave not found in $(KM_ENCLAVE_PATH) (check KM_ENCLAVE_PATH)" && \
		$(ECHO) "       Maybe you need to run \"make symlink-artifacts\" or \"make download-artifacts\"?" && \
		exit 1 \
	)

download-artifacts:
	@$(ECHO) "$(CYAN)*** Downloading Ekiden and runtime build artifacts...$(OFF)"
	@scripts/download_artifacts.sh "$(EKIDEN_ROOT_PATH)"
	@$(ECHO) "$(CYAN)*** Download completed!$(OFF)"

symlink-artifacts:
	@$(ECHO) "$(CYAN)*** Symlinking Ekiden and runtime build artifacts...$(OFF)"
	@export EKIDEN_CARGO_TARGET_DIR=$(EKIDEN_CARGO_TARGET_DIR) && \
		scripts/symlink_artifacts.sh "$(EKIDEN_ROOT_PATH)" "$(EKIDEN_SRC_PATH)" "$(RUNTIME_ROOT_PATH)" $$(pwd)
	@$(ECHO) "$(CYAN)*** Symlinking done!$(OFF)"

runtime: check-ekiden
	@$(ECHO) "$(CYAN)*** Building runtime-ethereum...$(OFF)"
	@export KM_ENCLAVE_PATH=$(KM_ENCLAVE_PATH) && \
		cargo build -p runtime-ethereum $(EXTRA_BUILD_ARGS) --target x86_64-fortanix-unknown-sgx && \
		cargo build -p runtime-ethereum $(EXTRA_BUILD_ARGS) && \
		cargo elf2sgxs $(EXTRA_BUILD_ARGS)

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
			"$(GENESIS_ROOT_PATH)/ekiden_$${g}"; \
	done

benchmark: genesis
	@$(ECHO) "$(CYAN)*** Building benchmark client...$(OFF)"
	@make -C benchmark

run-gateway:
	@$(ECHO) "$(CYAN)*** Starting Ekiden node and Web3 gateway...$(OFF)"
	@export EKIDEN_ROOT_PATH=$(EKIDEN_ROOT_PATH) RUNTIME_CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) && \
		scripts/gateway.sh single_node 2>&1 | python scripts/color-log.py

run-gateway-sgx:
	@$(ECHO) "$(CYAN)*** Starting Ekiden node and Web3 gateway (SGX)...$(OFF)"
	@export EKIDEN_ROOT_PATH=$(EKIDEN_ROOT_PATH) RUNTIME_CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) && \
		scripts/gateway.sh single_node_sgx 2>&1 | python scripts/color-log.py

test: test-unit test-e2e

test-unit: check-ekiden
	@$(ECHO) "$(CYAN)*** Running unit tests...$(OFF)"
	@export KM_ENCLAVE_PATH=$(KM_ENCLAVE_PATH) && \
		cargo test \
			--features test \
			-p runtime-ethereum-common \
			-p runtime-ethereum \
			-p web3-gateway
	@make -C benchmark test

test-e2e: check-ekiden
	@$(ECHO) "$(CYAN)*** Running E2E tests...$(OFF)"
	@.buildkite/scripts/download_ekiden_test_scripts.sh
	@export EKIDEN_ROOT_PATH=$(EKIDEN_ROOT_PATH) && \
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
