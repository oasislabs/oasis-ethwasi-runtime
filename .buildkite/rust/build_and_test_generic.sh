#!/bin/bash

############################################################
# This script builds a generic rust project and runs
# the tests for that project.
#
# Usage:
# build_and_test_generic.sh <src_dir>
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

#########################################
# Additional args passed to `cargo build`
#########################################
extra_args=$*

source .buildkite/rust/common.sh

#########################
# Run the build and tests
#########################
pushd $src_dir
  cargo build $extra_args
  cargo test $extra_args
popd
