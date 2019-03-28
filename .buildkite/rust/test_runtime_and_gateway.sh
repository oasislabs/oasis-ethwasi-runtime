#!/bin/bash

############################################################
# This script runs the runtime and gateway tests.
#
# Usage:
# test_runtime_and_gateway.sh <src_dir>
#
# src_dir - Absolute or relative path to the directory
#           containing the source code.
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
src_dir=$1
if [ ! -d $src_dir ]; then
  echo "ERROR: Invalid source directory specified (${src_dir})."
  exit 1
fi
shift

source .buildkite/rust/common.sh

#######################################
# Fetch the key manager runtime enclave
#######################################
echo "Fetching the ekiden-keymanager-runtime.sgxs enclave"
mkdir -p $src_dir/target/x86_64-fortanix-unknown-sgx/debug
.buildkite/scripts/download_artifact.sh \
    ekiden \
    $EKIDEN_BRANCH \
    "Build key manager runtime" \
    ekiden-keymanager-runtime.sgxs \
    $src_dir/target/x86_64-fortanix-unknown-sgx/debug

export KM_ENCLAVE_PATH="$src_dir/target/x86_64-fortanix-unknown-sgx/debug/ekiden-keymanager-runtime.sgxs"

###############
# Run the tests
###############
cd $src_dir
cargo test \
    --features test \
    -p runtime-ethereum \
    -p runtime-ethereum-common \
    -p web3-gateway
