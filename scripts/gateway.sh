#!/bin/bash

set -euo pipefail

# For automatic cleanup on exit.
source .buildkite/scripts/common.sh

config="$1"
ekiden_node="${EKIDEN_ROOT_PATH}/go/ekiden/ekiden"
web3_gateway="${RUNTIME_CARGO_TARGET_DIR}/debug/gateway"

# Prepare an empty node directory.
data_dir="/tmp/runtime-ethereum-${config}"
rm -rf "${data_dir}"
cp -R "configs/${config}" "${data_dir}"
chmod -R go-rwx "${data_dir}"

# Start the Ekiden node.
${ekiden_node} --config configs/${config}.yml &
sleep 1

# Start the gateway.
${web3_gateway} \
    --node-address "unix:${data_dir}/internal.sock" \
    --runtime-id 0000000000000000000000000000000000000000000000000000000000000000
