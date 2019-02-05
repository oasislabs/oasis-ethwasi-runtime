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

##############################
# Fetch the keymanager enclave
##############################
mkdir -p $src_dir/target/enclave

echo "Fetching the ekiden-keymanager-trusted.so enclave"
.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build key manager enclave" ekiden-keymanager-trusted.so $src_dir/target/enclave

export KM_ENCLAVE_PATH="$src_dir/target/enclave/ekiden-keymanager-trusted.so"

###############
# Run the tests
###############
cd $src_dir
cargo test --features test -p runtime-ethereum -p runtime-ethereum-common -p web3-gateway
