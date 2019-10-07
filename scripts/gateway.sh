#!/bin/bash

set -euo pipefail

# For automatic cleanup on exit.
source .buildkite/scripts/common.sh

ekiden_node="${EKIDEN_ROOT_PATH}/go/ekiden/ekiden"
ekiden_runner="${EKIDEN_ROOT_PATH}/go/ekiden-net-runner/ekiden-net-runner"
runtime_binary="${RUNTIME_CARGO_TARGET_DIR}/debug/runtime-ethereum"
runtime_loader="${EKIDEN_ROOT_PATH}/target/debug/ekiden-runtime-loader"
runtime_genesis="${GENESIS_ROOT_PATH}/ekiden_genesis_testing.json"
keymanager_binary="${EKIDEN_ROOT_PATH}/target/debug/ekiden-keymanager-runtime"
web3_gateway="${RUNTIME_CARGO_TARGET_DIR}/debug/gateway"

# Prepare an empty data directory.
data_dir="/tmp/runtime-ethereum-runner"
rm -rf "${data_dir}"
mkdir -p "${data_dir}"
chmod -R go-rwx "${data_dir}"
client_socket="${data_dir}/net-runner/network/client-0/internal.sock"


# Run the network.
${ekiden_runner} \
    --net.ekiden.binary ${ekiden_node} \
    --net.runtime.binary ${runtime_binary} \
    --net.runtime.loader ${runtime_loader} \
    --net.runtime.genesis_state ${runtime_genesis} \
    --net.keymanager.binary ${keymanager_binary} \
    --basedir.no_temp_dir \
    --basedir ${data_dir} &

# Wait for the nodes to be registered.
${ekiden_node} debug dummy wait-nodes \
    --address unix:${client_socket} \
    --nodes 6

# Start the gateway.
${web3_gateway} \
    --node-address unix:${client_socket} \
    --runtime-id 0000000000000000000000000000000000000000000000000000000000000000 \
    --ws-port 8555
