#!/bin/bash

set -euo pipefail

# For automatic cleanup on exit.
source .buildkite/scripts/common.sh

oasis_node="${OASIS_CORE_ROOT_PATH}/go/oasis-node/oasis-node"
oasis_runner="${OASIS_CORE_ROOT_PATH}/go/oasis-net-runner/oasis-net-runner"
runtime_binary="${RUNTIME_CARGO_TARGET_DIR}/debug/oasis-runtime"
runtime_loader="${OASIS_CORE_ROOT_PATH}/target/debug/oasis-core-runtime-loader"
runtime_genesis="${GENESIS_ROOT_PATH}/ekiden_genesis_testing.json"
keymanager_binary="${OASIS_CORE_ROOT_PATH}/target/debug/oasis-core-keymanager-runtime"
web3_gateway="${RUNTIME_CARGO_TARGET_DIR}/debug/gateway"

# Prepare an empty data directory.
data_dir="/tmp/oasis-runtime-runner"
rm -rf "${data_dir}"
mkdir -p "${data_dir}"
chmod -R go-rwx "${data_dir}"
client_socket="${data_dir}/net-runner/network/client-0/internal.sock"


# Run the network.
${oasis_runner} \
    --net.node.binary ${oasis_node} \
    --net.runtime.binary ${runtime_binary} \
    --net.runtime.loader ${runtime_loader} \
    --net.runtime.genesis_state ${runtime_genesis} \
    --net.keymanager.binary ${keymanager_binary} \
    --basedir.no_temp_dir \
    --basedir ${data_dir} &

# Wait for the nodes to be registered.
${oasis_node} debug dummy wait-nodes \
    --address unix:${client_socket} \
    --nodes 6

# Start the gateway.
${web3_gateway} \
    --node-address unix:${client_socket} \
    --runtime-id 0000000000000000000000000000000000000000000000000000000000000000 \
    --ws-port 8555
