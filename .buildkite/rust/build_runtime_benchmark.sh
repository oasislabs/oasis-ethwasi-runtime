#!/bin/bash

#################################################
# This script builds the runtime benchmark.
#
# Usage:
# build_runtime_benchmark.sh <src_dir>
#
# src_dir - Absolute or relative path to the
#           directory containing the source code.
#################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
src_dir=$1

source .buildkite/rust/common.sh

# Temporary artifacts directory
ARTIFACTS_DIR=/tmp/artifacts

#################################
# Change into the build directory
#################################
cd $src_dir

# Install ekiden-tools
echo "Installing ekiden-tools."
cargo install \
  --git https://github.com/oasislabs/ekiden \
  --branch master \
  --debug \
  ekiden-tools

###############################################
# Build the benchmarking version of the runtime
###############################################
cargo ekiden build-enclave \
  --output-identity \
  --release \
  --cargo-addendum feature.benchmark.addendum \
  --out-dir ${ARTIFACTS_DIR} \
  -- \
  --features "benchmark"

######################################################
# Taken from docker/benchmarking/build-images-inner.sh
######################################################

# Build all Ekiden binaries and resources.
pushd benchmark
  make
  cp benchmark ${ARTIFACTS_DIR}
popd

cargo build -Z unstable-options -p web3-gateway genesis --release --out-dir ${ARTIFACTS_DIR}

# Package all binaries and resources.
mkdir -p target/docker-benchmarking/context/bin target/docker-benchmarking/context/lib target/docker-benchmarking/context/res
pushd ${ARTIFACTS_DIR}
  ln runtime-ethereum.so target/docker-benchmarking/context/lib/runtime-ethereum-benchmarking.so
  ln runtime-ethereum.mrenclave target/docker-benchmarking/context/res/runtime-ethereum-benchmarking.mrenclave
  ln benchmark target/docker-benchmarking/context/bin
  ln gateway target/docker-benchmarking/context/bin
  ln genesis target/docker-benchmarking/context/bin
popd

ln docker/benchmarking/Dockerfile target/docker-benchmarking/context/Dockerfile
tar cvzhf target/docker-benchmarking/context.tar.gz -C target/docker-benchmarking/context .
rm -rf target/docker-benchmarking/context
