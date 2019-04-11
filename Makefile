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
	clean clean-test-e2e \
	fmt \
	benchmark \
	run-gateway run-gateway-sgx \
	test test-unit test-e2e

all: check runtime gateway benchmark
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
		cargo build -p runtime-ethereum --target x86_64-fortanix-unknown-sgx && \
		cargo build -p runtime-ethereum && \
		cargo elf2sgxs

gateway:
	@$(ECHO) "$(CYAN)*** Building web3-gateway...$(OFF)"
	@cargo build -p web3-gateway

genesis:
	@$(ECHO) "$(CYAN)*** Building replay benchmark genesis utility...$(OFF)"
	@cargo build -p genesis

benchmark: genesis
	@$(ECHO) "$(CYAN)*** Building benchmark client...$(OFF)"
	@make -C benchmark

run-gateway:
	@$(ECHO) "$(CYAN)*** Starting Ekiden node and Web3 gateway...$(OFF)"
	@export EKIDEN_ROOT_PATH=$(EKIDEN_ROOT_PATH) RUNTIME_CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) && \
		scripts/gateway.sh single_node

run-gateway-sgx:
	@$(ECHO) "$(CYAN)*** Starting Ekiden node and Web3 gateway (SGX)...$(OFF)"
	@export EKIDEN_ROOT_PATH=$(EKIDEN_ROOT_PATH) RUNTIME_CARGO_TARGET_DIR=$(RUNTIME_CARGO_TARGET_DIR) && \
		scripts/gateway.sh single_node_sgx

test: test-unit test-e2e

test-unit: check-ekiden
	@$(ECHO) "$(CYAN)*** Running unit tests...$(OFF)"
	@export KM_ENCLAVE_PATH=$$(KM_ENCLAVE_PATH) && \
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
	@make -C benchmark fmt

clean-test-e2e:
	@$(ECHO) "$(CYAN)*** Cleaning up E2E tests...$(OFF)"
	@rm -rf .e2e
	@rm -rf tests/rpc-tests
	@rm -rf tests/e2e-tests

clean: clean-test-e2e
	@$(ECHO) "$(CYAN)*** Cleaning up...$(OFF)"
	@cargo clean