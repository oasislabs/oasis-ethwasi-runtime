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

EKIDEN_UNSAFE_SKIP_AVR_VERIFY=1
export EKIDEN_UNSAFE_SKIP_AVR_VERIFY

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
export KM_ENCLAVE_PATH="$platform_dir/ekiden-keymanager-trusted.so"

cargo build --release ${RUNTIME_BUILD_EXTRA_ARGS:-}
cargo build --release --target x86_64-fortanix-unknown-sgx ${RUNTIME_BUILD_EXTRA_ARGS:-}
cargo elf2sgxs

# Build the gateway
(
    cd gateway
    cargo build -p web3-gateway \
        --release \
        ${GATEWAY_BUILD_EXTRA_ARGS:-}
)

tar -czf "$dst" \
    target/release/runtime-ethereum \
    target/x86_64-fortanix-unknown-sgx/release/runtime-ethereum.sgxs \
    target/release/gateway \
    docker/ekiden-runtime-ethereum/Dockerfile
