#!/bin/bash

# TODO Update build scripts to be DRY.

##################################################
# Simple wrapper script to call
# scripts/test-dapp.sh
# with the correct arguments.
#
# Downloads all of the required build artifacts
# to run the tests and makes sure they are in the
# correct directories, etc.
#
# This script is intended to have buildkite
# specific things, like env vars and calling
# the buildkite-agent binary. Keeping this
# separate from the generic script that gets
# called allows us to use and test the generic
# scripts easily on a local dev box.
##################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

####################
# Required arguments
####################
test=$1

################
# Ensure cleanup
################
source scripts/utils.sh
trap 'cleanup' EXIT

####################################################
# By default, .bashrc will quit if the shell
# is not interactive. It checks whether $PS1 is
# set to determine whether the shell is interactive.
# Here, we set PS1 to any random value so that we
# can source .bashrc and have it configure $PATH
# for things like node version manager (nvm) and
# sgxsdk.
####################################################
# TODO this is very unintuitive. Think of a better way to do this.
export PS1="set PS1 to anything so that we can source .bashrc"

# While sourcing .bashrc, temporarily ignore
# unset vars and do not print commands because
# it is a bunch of useless noise.
set +ux
. ~/.bashrc
set -ux

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

######################################################
# Install ekiden-compue if it is not already installed
######################################################
set +u
cargo_install_root=$(get_cargo_install_root)
echo "cargo_install_root=$cargo_install_root"
set -u

if [ ! -e "$cargo_install_root/bin/ekiden-compute" ]; then
  echo "Installing ekiden-compute."  
  cargo install \
    --git https://github.com/oasislabs/ekiden \
    --branch master \
    --debug \
    ekiden-compute
fi

# Run the ens tests
./scripts/test-dapp.sh ${test}
