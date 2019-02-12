#!/bin/bash

################################################################################
# Runs a local testnet with the latest build artifacts from CI.
#
# Steps to run:
#
# - export BUILDKITE_ACCESS_TOKEN=[YOUR_ACCESS_TOKEN]
#   You can get one of these from your personal buildkite account through the
#   web ui. See https://buildkite.com/user/api-access-tokens.
# - apt-get install jq
# - ./scripts/oasis.sh
#
# You now have a local network running.
#
# To force download the artifacts, make sure to wipe the OASIS_HOME_DIR.
#
# Usage:
# ./scripts/oasis.sh [RUN_TESTNET]
#
# Optional Args:
# - RUN_TESTNET: True by default. If true, then the script will block and start
#                the network. If false, will just download the Oasis binaries
#                into the OASIS_HOME_DIR.
#
################################################################################

RUN_TESTNET=${1:-true}
WORKDIR=$(pwd)

# Directory we want to save the build artifacts in.
OASIS_HOME_DIR="/tmp/oasis"
OASIS_ARTIFACTS_DIR="${OASIS_HOME_DIR}/artifacts"
# Branches we want to download the artifacts from.
EKIDEN_BRANCH="master"
RUNTIME_BRANCH="master"

# Download all the artifacts that we need to run a local network,
# if they don't already exist.
if [ ! -d "$OASIS_ARTIFACTS_DIR" ]; then
	mkdir -p $OASIS_ARTIFACTS_DIR
	source .buildkite/scripts/download_utils.sh
	download_oasis_binaries $OASIS_ARTIFACTS_DIR
fi

source scripts/utils.sh

# Define these so that we override the paths define in scripts.utils.sh.
export EKIDEN_NODE=$OASIS_ARTIFACTS_DIR/ekiden-node
export EKIDEN_WORKER=$OASIS_ARTIFACTS_DIR/ekiden-worker
export KM_ENCLAVE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-trusted.so
export KM_MRENCLAVE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-trusted.mrenclave
export KM_NODE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-node
export GATEWAY=$OASIS_ARTIFACTS_DIR/gateway
export RUNTIME_ENCLAVE=$OASIS_ARTIFACTS_DIR/runtime-ethereum.so
export RUNTIME_MRENCLAVE=$OASIS_ARTIFACTS_DIR/runtime-ethereum.mrenclave

trap 'cleanup' EXIT

if [ $RUN_TESTNET = "true" ]; then
	run_test_network
	wait
fi
