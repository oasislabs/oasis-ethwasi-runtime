#!/bin/bash

# Build a Docker context tarball.

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
platform_dir=$1
dst=$2

: ${SGX_MODE:=SIM}
export SGX_MODE
EKIDEN_UNSAFE_SKIP_AVR_VERIFY=1
export EKIDEN_UNSAFE_SKIP_AVR_VERIFY
: ${INTEL_SGX_SDK:=/opt/sgxsdk}
export INTEL_SGX_SDK

# Install ekiden-tools
#
# TODO: There is no need to continuously reinstall ekiden-tools
#       all over the place. Instead create an image ekiden/builder
#       or something like that and then use that as the base image
#       for this and other runtime-ethereum CI jobs.
cargo install \
    --force \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    ekiden-tools

# Build the runtime
KM_ENCLAVE_PATH="$platform_dir/ekiden-keymanager-trusted.so" \
cargo ekiden build-enclave \
    --output-identity \
    --release \
    ${RUNTIME_BUILD_EXTRA_ARGS:-}

# Build the gateway
(
    cd gateway
    cargo build -p web3-gateway \
        --release \
        ${GATEWAY_BUILD_EXTRA_ARGS:-}
)

tar -czf "$dst" \
    target/enclave/runtime-ethereum.so \
    target/enclave/runtime-ethereum.mrenclave \
    target/release/gateway \
    docker/ekiden-runtime-ethereum/Dockerfile
