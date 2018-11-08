#!/bin/bash

# TODO Update build scripts to be DRY.

##
# This script runs the end to end tests.
# 
# Dependencies from other jobs are required to run the tests.
# The dependencies from other jobs are as follows:
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
# run_end_to_end_tests.sh [src_dir]
#
# src_dir - the path to the directory containing the source
#           code. This value SHOULD NOT end in a slash.
#           (TODO: add input validation to remove trailing slashes)
##

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

# By default, .bashrc will quit if the shell
# is not interactive. It checks whether $PS1 is
# set to determine whether the shell is interactive.
# Here, we set PS1 to any random value so that we
# can source .bashrc and have it configure $PATH
# for things like node version manager (nvm) and
# sgxsdk.
# TODO this is very unintuitive. Think of a better way to do this.
export PS1="set PS1 to anything so that we can source .bashrc"

# While sourcing .bashrc, temporarily ignore
# unset vars and do not print commands because
# it is a bunch of useless noise.
set +ux
. ~/.bashrc
set -ux

# Set up environment
export SGX_MODE=SIM
export INTEL_SGX_SDK=/opt/sgxsdk
export EKIDEN_UNSAFE_SKIP_AVR_VERIFY=1

# Add SSH identity so that `cargo build`
# can successfully download dependencies
# from private github repos.
eval `ssh-agent -s`
ssh-add

# Add github public key to known_hosts.
# This is required because some test scripts
# run `npm install` and at least one dependency
# has its own dependencies that pull from
# GitHub and the /root/.gitconfig file transforms
# https to ssh when pulling from GitHub.
ssh-keyscan rsa github.com >> ~/.ssh/known_hosts

# Update the PATH to respect $CARGO_INSTALL_ROOT.
# This allows 'cargo install' to reuse binaries 
# from previous installs as long as the correct
# host directory is mounted on the docker container.
# Huge speed improvements during local dev and testing.
set +u
export PATH=$CARGO_INSTALL_ROOT/bin/:$PATH
set -u

# Run setup script
./scripts/setup-e2e.sh

# Run gateway RPC tests
./scripts/test-rpc.sh

# # Run web3.js pubsub test
./scripts/test-pubsub.sh

# # Run the basic wasm contract test
./scripts/test-basic-wasm.sh

# # Run the storage contract test
./scripts/test-storage.sh

# # Run the rust logistic contract test
./scripts/test_rust_logistic.sh

# # Run the end-to-end test
./scripts/test-e2e.sh
