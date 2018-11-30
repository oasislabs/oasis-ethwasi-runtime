#!/bin/bash

#################################################
# This script runs the end to end tests.
#
# Dependencies from other jobs are required to
# run the tests. The dependencies from other
# jobs are as follows:
#
# - job: build-and-test-runtime
#   dependencies:
#     - target/enclave/runtime-ethereum.so
#     - target/enclave/runtime-ethereum.mrenclave
# - job: build-oasislabs-ekiden-go
#   dependencies:
#     - /go/bin/ekiden as ekiden-node
# - job: build-and-test-web3-gateway
#   dependencies:
#     - target/debug/gateway
#
# Usage:
# run_end_to_end_tests.sh
#################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source .buildkite/rust/common.sh

#################################################
# Add github public key to known_hosts.
# This is required because some test scripts
# run `npm install` and at least one dependency
# has its own dependencies that pull from
# GitHub and the /root/.gitconfig file transforms
# https to ssh when pulling from GitHub.
#################################################
ssh-keyscan rsa github.com >> ~/.ssh/known_hosts

#######################################################
# Update the PATH to respect $CARGO_INSTALL_ROOT.
# This allows 'cargo install' to reuse binaries
# from previous installs as long as the correct
# host directory is mounted on the docker container.
# Huge speed improvements during local dev and testing.
#######################################################
set +u
export PATH=$CARGO_INSTALL_ROOT/bin/:$PATH
set -u

# Run setup script
./scripts/setup-e2e.sh

# Run gateway RPC tests
./scripts/test-rpc.sh

# Run the web3c.js tests
./scripts/test-web3cjs.sh

# Run web3.js pubsub test
./scripts/test-pubsub.sh

# Run the basic wasm contract test
./scripts/test-basic-wasm.sh

# Run the storage contract test
./scripts/test-storage.sh

# Run the rust logistic contract test
./scripts/test_rust_logistic.sh

# Run the end-to-end test
./scripts/test-e2e.sh
