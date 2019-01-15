#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Paths to Go node and keymanager enclave, assuming they were built according to the README
EKIDEN_NODE=/go/src/github.com/oasislabs/ekiden/go/ekiden/ekiden
KM_MRENCLAVE=/go/src/github.com/oasislabs/ekiden/target/enclave/ekiden-keymanager-trusted.mrenclave
KM_ENCLAVE=/go/src/github.com/oasislabs/ekiden/target/enclave/ekiden-keymanager-trusted.so

# Paths to ekiden binaries
EKIDEN_WORKER=$(which ekiden-worker)
KM_NODE=$(which ekiden-keymanager-node)

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

# Start keymanager node.
run_keymanager_node
sleep 1

# Start validator committee.
run_backend_tendermint_committee
sleep 1

# Start compute nodes.
run_compute_node 1
sleep 1
run_compute_node 2
sleep 2

# Advance epoch to elect a new committee.
${EKIDEN_NODE} debug dummy set-epoch --epoch 1

# Start the gateway.
echo "Starting web3 gateway."
run_gateway 1
wait
