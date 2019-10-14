#! /bin/bash

###############################################
# Download common build artifacts and make sure
# they are in the correct directories for tests
# to run, etc, etc.
#
# This script is intended to have buildkite
# specific things, like env vars and calling
# the buildkite-agent binary. Keeping this
# separate from the generic script that gets
# called allows us to use and test the generic
# scripts easily on a local dev box.
###############################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source .buildkite/scripts/download_utils.sh

# Create directory to put artifacts into.
mkdir -p \
    go/oasis-node \
    go/oasis-net-runner \
    go/developer-gateway \
    target/debug \
    target/x86_64-fortanix-unknown-sgx/debug

###########################################
# Download artifacts from other pipelines
###########################################
download_oasis_node go/oasis-node
download_oasis_net_runner go/oasis-net-runner
download_oasis_core_runtime_loader target/debug
download_keymanager_runtime target/debug
download_keymanager_runtime_sgx target/x86_64-fortanix-unknown-sgx/debug
download_developer_gateway go/developer-gateway

########################
# Download oasis-runtime
########################
buildkite-agent artifact download \
    oasis-runtime.sgxs \
    target/x86_64-fortanix-unknown-sgx/debug

buildkite-agent artifact download \
    oasis-runtime \
    target/debug
chmod +x target/debug/oasis-runtime

##################
# Download gateway
##################
mkdir -p target/debug
buildkite-agent artifact download \
    gateway \
    target/debug
chmod +x target/debug/gateway
