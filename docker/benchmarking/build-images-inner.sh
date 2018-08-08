#!/bin/bash -e

# Our development image sets up the PATH in .bashrc. Source that.
PS1='\$'
. ~/.bashrc
set -x

# Abort on unclean packaging area.
if [ -e target/docker-benchmarking/context ]; then
    cat >&2 <<EOF
Path target/docker-benchmarking/context already exists. Aborting.
If this was accidentally left over and you don't need anything from
it, you can remove it and try again.
EOF
    exit 1
fi

# Build all Ekiden binaries and resources.
CARGO_TARGET_DIR=target cargo install --force --git https://github.com/oasislabs/ekiden --branch master ekiden-tools
cargo ekiden build-enclave --output-identity --release --cargo-addendum feature.benchmark.addendum -- --features "benchmark"
(cd gateway && CARGO_TARGET_DIR=../target cargo build --release)
(cd genesis && CARGO_TARGET_DIR=../target cargo build --release)
(cd playback && CARGO_TARGET_DIR=../target cargo build --release)

# Package all binaries and resources.
mkdir -p target/docker-benchmarking/context/bin target/docker-benchmarking/context/lib target/docker-benchmarking/context/res
ln target/enclave/runtime-ethereum.so target/docker-benchmarking/context/lib/runtime-ethereum-benchmarking.so
ln target/enclave/runtime-ethereum.mrenclave target/docker-benchmarking/context/res/runtime-ethereum-benchmarking.mrenclave
ln target/release/gateway target/docker-benchmarking/context/bin
ln target/release/genesis target/docker-benchmarking/context/bin
ln target/release/playback target/docker-benchmarking/context/bin
ln docker/benchmarking/Dockerfile target/docker-benchmarking/context/Dockerfile
tar cvzhf target/docker-benchmarking/context.tar.gz -C target/docker-benchmarking/context .
rm -rf target/docker-benchmarking/context
