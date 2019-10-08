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

OASIS_UNSAFE_SKIP_AVR_VERIFY=1
export OASIS_UNSAFE_SKIP_AVR_VERIFY

# Install oasis-core-tools.
cargo install \
    --force \
    --git https://github.com/oasislabs/oasis-core \
    --branch master \
    oasis-core-tools

# Build the runtime.
cargo build --release ${RUNTIME_BUILD_EXTRA_ARGS:-}
cargo build --release --target x86_64-fortanix-unknown-sgx ${RUNTIME_BUILD_EXTRA_ARGS:-}
cargo elf2sgxs --release

# Build the gateway.
(
    cd gateway
    cargo build -p web3-gateway \
        --release \
        ${GATEWAY_BUILD_EXTRA_ARGS:-}
)

# Copy the correct genesis file.
if [ -n "${BUILD_PRODUCTION_GENESIS:-}" ]; then
    cp resources/genesis/oasis_genesis.json resources/genesis.json
else
    cp resources/genesis/oasis_genesis_testing.json resources/genesis.json
fi

tar -czf "$dst" \
    resources/genesis.json \
    target/release/oasis-runtime \
    target/x86_64-fortanix-unknown-sgx/release/oasis-runtime.sgxs \
    target/release/gateway \
    docker/deployment/Dockerfile
