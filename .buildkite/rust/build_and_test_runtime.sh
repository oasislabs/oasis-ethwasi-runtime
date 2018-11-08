#!/bin/bash

# TODO Update build scripts to be DRY.

##
# This script builds the runtime and runs the runtime tests.
# 
# Usage:
# build_rust_runtime.sh [src_dir]
#
# src_dir - the path to the directory containing the source
#           code. This value SHOULD NOT end in a slash.
#           (TODO: add input validation to remove trailing slashes)
#
# build_sub_dir - a relative subpath of src_dir that should
#                 be made the working directory within build_dir
#                 before building.
##

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

src_dir=$1
build_sub_dir=$2

source scripts/utils.sh

# Set up environment
export SGX_MODE=SIM
export INTEL_SGX_SDK=/opt/sgxsdk
export EKIDEN_UNSAFE_SKIP_AVR_VERIFY=1

# Add SSH identity so that `cargo build`
# can successfully download dependencies
# from private github repos.
# TODO kill this process when script exits
eval `ssh-agent -s`
ssh-add

# Change into the build directory
cd $src_dir
cd $build_sub_dir

# Apply the rust code formatting rules
cargo fmt -- --write-mode=check

# Update the PATH to respect $CARGO_INSTALL_ROOT.
# This allows 'cargo install' to reuse binaries 
# from previous installs as long as the correct
# host directory is mounted on the docker container.
# Huge speed improvements during local dev and testing.
set +u
export PATH=$CARGO_INSTALL_ROOT/bin/:$PATH
set -u

# Only run 'cargo install' if the resulting binaries
# are not already present. The 'cargo install' command
# will error out if the binary is already installed.
# Making this script idempotent is really useful for
# local development and testing.
set +u
cargo_install_root=$(get_cargo_install_root)
echo "cargo_install_root = $cargo_install_root"
set -u

if [ ! -e "$cargo_install_root/bin/cargo-ekiden" ]; then
  echo "Installing ekiden-tools."
  cargo install \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    --debug \
    ekiden-tools
fi

# Build the runtime
cargo ekiden build-enclave --output-identity

# Run the tests
cargo test
