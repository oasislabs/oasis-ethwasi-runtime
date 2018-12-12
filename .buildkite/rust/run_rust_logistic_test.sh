#!/bin/bash

#################################################
# This script runs the Rust logistic test.
#
# Dependencies from other jobs are required to
# run the test. The dependencies from other
# jobs are as follows:
#
# - job: build-keymanager-enclave
#   dependencies:
#     - target/enclave/ekiden-keymanager-trusted.so
#     - target/enclave/ekiden-keymanager-trusted.mrenclave
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
# run_rust_logistic_test.sh
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

# Run the rust logistic contract test
./scripts/test_rust_logistic.sh

