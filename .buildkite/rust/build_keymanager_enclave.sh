#!/bin/bash

############################################################
# This script builds the runtime and runs the runtime tests.
#
# Usage:
# build_and_test_runtime.sh <src_dir>
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

#################################
# Change into the build directory
#################################
cd $src_dir

#######################################################
# Update the PATH to respect $CARGO_INSTALL_ROOT.
# This allows 'cargo install' to reuse binaries
# from previous installs as long as the correct
# host directory is mounted on the docker container.
# Huge speed improvements during local dev and testing.
#######################################################
set +u
export PATH=$CARGO_INSTALL_ROOT/bin/:$PATH
set -u

echo "Installing ekiden-tools."
cargo install \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    --debug \
    ekiden-tools

###################
# Build the enclave
###################

git clone https://github.com/oasislabs/ekiden.git
cd ekiden/key-manager/dummy/enclave
cargo ekiden build-enclave --output-identity ${extra_args}
