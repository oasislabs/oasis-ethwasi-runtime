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
.buildkite/scripts/download_artifact.sh ekiden master "Build Go node" ekiden .
mv ekiden ekiden-node
chmod +x ekiden-node

.buildkite/scripts/download_artifact.sh ekiden master "Build Rust worker, compute node and key manager node" ekiden-worker .
chmod +x ekiden-worker

.buildkite/scripts/download_artifact.sh ekiden master "Build Rust worker, compute node and key manager node" ekiden-keymanager-node .
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
# Download ekiden-keymanager-trusted.mrenclave
#####################################################
.buildkite/scripts/download_artifact.sh ekiden master "Build key manager enclave" ekiden-keymanager-trusted.mrenclave target/enclave
.buildkite/scripts/download_artifact.sh ekiden master "Build key manager enclave" ekiden-keymanager-trusted.so target/enclave

##################
# Download gateway
##################
mkdir -p target/debug
buildkite-agent artifact download \
    gateway \
    target/debug
chmod +x target/debug/gateway
