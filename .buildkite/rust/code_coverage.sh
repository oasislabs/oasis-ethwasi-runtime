#!/bin/bash

# TODO Update build scripts to be DRY.

#################################################
# This script uses Tarpaulin to calculate test
# coverage in the code base.
#
# Usage:
# code_coverage.sh [path_to_coveralls_api_token]
#
# path_to_coveralls_api_token - Absolute or relative
#     path to a file that contains the coveralls.io
#     API token. Defaults to "~/.coveralls/api_token".
#################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

###############
# Optional args
###############
path_to_coveralls_api_token=${1:-~/.coveralls/api_token}

############
# Local vars
############
coveralls_api_token=$(cat ${path_to_coveralls_api_token})

####################
# Set up environment
####################
export SGX_MODE="SIM"
export INTEL_SGX_SDK="/opt/sgxsdk"
export EKIDEN_UNSAFE_SKIP_AVR_VERIFY="1"
export RUST_BACKTRACE="1"

########################################
# Add SSH identity so that `cargo build`
# can successfully download dependencies
# from private github repos.
# TODO kill this process when script exits
########################################
eval `ssh-agent -s`
ssh-add

#################################################
# Add github public key to known_hosts.
# This is required because some test scripts
# run `npm install` and at least one dependency
# has its own dependencies that pull from
# GitHub and the /root/.gitconfig file transforms
# https to ssh when pulling from GitHub.
#################################################
ssh-keyscan rsa github.com >> ~/.ssh/known_hosts

# Instal Tarpaulin
RUSTFLAGS="--cfg procmacro2_semver_exempt" \
  cargo install \
  --git https://github.com/oasislabs/tarpaulin \
  --branch ekiden \
  cargo-tarpaulin

# Workaround to avoid linker errors in
# tarpaulin: disable cargo build script.
echo 'fn main() {}' > build.rs

# Calculate coverage
cargo tarpaulin \
  --packages runtime-ethereum \
  --packages runtime-ethereum-common \
  --packages web3-gateway \
  --exclude-files *generated* \
  --exclude-files genesis* \
  --exclude-files node_modules* \
  --ignore-tests \
  --out Xml \
  --coveralls ${coveralls_api_token} \
  -v
