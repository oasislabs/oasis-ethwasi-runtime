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

source .buildkite/rust/common.sh

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

# Install ekiden-compute
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

# Run the ens tests
./scripts/test-dapp.sh ${test}
