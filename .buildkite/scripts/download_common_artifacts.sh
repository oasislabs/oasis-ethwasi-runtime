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

######################
# Download ekiden-node
######################
buildkite-agent artifact download ekiden-node .
chmod +x ekiden-node
buildkite-agent artifact download ekiden-compute .
chmod +x ekiden-compute
buildkite-agent artifact download ekiden-worker .
chmod +x ekiden-worker
buildkite-agent artifact download ekiden-keymanager-node .
chmod +x ekiden-keymanager-node

############################################
# Download runtime-ethereum(.so|.mrenclave)
############################################
mkdir -p target/enclave
buildkite-agent artifact download \
    runtime-ethereum.so \
    target/enclave
buildkite-agent artifact download \
    runtime-ethereum.mrenclave \
    target/enclave

#####################################################
# Download ekiden-keymanager-trusted(.so|.mrenclave)
#####################################################

buildkite-agent artifact download \
    ekiden-keymanager-trusted.so \
    target/enclave
buildkite-agent artifact download \
    ekiden-keymanager-trusted.mrenclave \
    target/enclave

##################
# Download gateway
##################
mkdir -p target/debug
buildkite-agent artifact download \
    gateway \
    target/debug
chmod +x target/debug/gateway
