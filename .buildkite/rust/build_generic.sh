#!/bin/bash

# TODO Update build scripts to be DRY.

############################################################
# This script builds a generic rust project.
# 
# Usage:
# build_generic.sh <src_dir>
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

#################################
# Change into the build directory
#################################
cd $src_dir

###############
# Run the build
###############
cargo build
