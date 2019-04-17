#!/bin/bash

# Build a Docker context tarball.

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
dst=$1

# Set protobuf branch to master, otherwise go-ethereum make fails, because
# git pull on protobuf fails.
pushd /go/src/github.com/golang/protobuf
    git checkout master
popd

# Compile and install go-ethereum devtools (abigen etc.).
go get -d github.com/ethereum/go-ethereum
pushd /go/src/github.com/ethereum/go-ethereum
    make devtools
popd

# Install cargo wasm32-unknown-unknown target and wasm build utilities.
rustup target add wasm32-unknown-unknown
cargo install owasm-utils-cli --bin wasm-build
apt install -y xxd

# Compile genesis tool.
cargo build -p genesis --release

# Ensure the CARGO_TARGET_DIR is not set so that oasis-compile can generate the
# correct rust contract artifacts. Can remove this once the following is
# addressed: https://github.com/oasislabs/oasis-compile/issues/44
unset CARGO_TARGET_DIR
# Ensure no special compiler flags are in effect.
unset RUSTFLAGS

# Compile benchmarking client containing benchmarks and benchmarking smart
# contracts written in rust.
make -C benchmark

tar -czf "$dst" \
    target/release/genesis-playback \
    benchmark/benchmark \
    docker/benchmarking/Dockerfile
