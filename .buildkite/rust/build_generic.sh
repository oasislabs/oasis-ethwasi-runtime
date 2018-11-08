#!/bin/bash

# TODO Update build scripts to be DRY.

##
# This script builds a rust project. 
# 
# Usage:
# build_rust_generic.sh [src_dir]
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

# Set up environment
export SGX_MODE=SIM
export INTEL_SGX_SDK=/opt/sgxsdk
export EKIDEN_UNSAFE_SKIP_AVR_VERIFY=1

# Add SSH identity so that `cargo build`
# can successfully download dependencies
# from private github repos.
eval `ssh-agent -s`
ssh-add

# Change into the build directory
cd $src_dir
cd $build_sub_dir

# Apply the rust code formatting rules
cargo fmt -- --write-mode=check

# Run the build
cargo build
