#!/bin/bash

#################################################
# This script runs the end to end test.
#
# Dependencies from other jobs are required to
# run the test. The dependencies from other
# jobs are as follows:
#
# - job: build-and-test-runtime
#   dependencies:
#     - target/enclave/runtime-ethereum.so
#     - target/enclave/runtime-ethereum.mrenclave
# - job: build-oasislabs-ekiden-go
#   dependencies:
#     - /go/bin/ekiden as ekiden-node
# - job: build-and-test-web3-gateway
#   dependencies:
#     - target/debug/gateway
#
# Usage:
# run_end_to_end_test.sh
#################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

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

# Run setup script
./scripts/setup-e2e.sh

echo "Downloading compiled contracts from the e2e-tests pipeline"
.buildkite/scripts/download_artifact.sh e2e-tests $E2E_TESTS_BRANCH "Lint and Compile Contracts" build.zip /e2e-tests

# Replace the contracts with the prebuilt ones
pushd /e2e-tests/ > /dev/null

unzip build.zip -d .

popd > /dev/null

# Ensures we don't try to compile the contracts a second time.
export SKIP_OASIS_COMPILE=true

# Re-export parallelism parameters so that they can be read by the e2e-tests.
export E2E_PARALLELISM=${BUILDKITE_PARALLEL_JOB_COUNT}
export E2E_PARALLELISM_BUCKET=${BUILDKITE_PARALLEL_JOB}

# Run the end-to-end test
./scripts/test-e2e.sh
