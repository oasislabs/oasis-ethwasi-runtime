#!/bin/bash

############################################################
# This script builds the ekiden binaries.
#
# Usage:
# build_ekiden_binaries.sh <out_dir>
#
# out_dir - Absolute or relative path to the directory
#           where the built binaries are stored.
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
out_dir=$1

source .buildkite/rust/common.sh

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

echo "Installing ekiden-compute."
cargo install \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    --debug \
    ekiden-compute

echo "Installing ekiden-worker."
cargo install \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    --debug \
    ekiden-worker

echo "Installing ekiden-keymanager-node."
cargo install \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    --debug \
    ekiden-keymanager-node
