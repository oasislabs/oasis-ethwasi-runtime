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

###############
# Run the tests
###############
cd $src_dir
cargo test \
    --features test \
    -p oasis-runtime \
    -p oasis-runtime-common \
    -p oasis-runtime-keymanager \
    -p web3-gateway
