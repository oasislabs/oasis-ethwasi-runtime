#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Paths to dummy node and keymanager enclave, assuming they were built according to the README
DUMMY_NODE=/go/src/github.com/oasislabs/ekiden/go/ekiden/ekiden
KM_MRENCLAVE=/go/src/github.com/oasislabs/ekiden/target/enclave/ekiden-keymanager-trusted.mrenclave
KM_ENCLAVE=/go/src/github.com/oasislabs/ekiden/target/enclave/ekiden-keymanager-trusted.so

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_dummy_node() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ${DUMMY_NODE} \
        --log.level debug \
        --grpc.port 42261 \
        --epochtime.backend tendermint_mock \
        --beacon.backend insecure \
        --storage.backend memory \
        --scheduler.backend trivial \
        --registry.backend tendermint \
        --roothash.backend tendermint \
        --tendermint.consensus.timeout_commit 250ms \
        --datadir ${datadir} \
        &> dummy.log &
}

run_test() {
    local dummy_node_runner=$1

    # Start keymanager node
    run_keymanager_node
    sleep 1

    # Start dummy node.
    $dummy_node_runner
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2
    sleep 2

    # Advance epoch to elect a new committee.
    ekiden debug dummy set-epoch --epoch 1

    # Run the client. We run the client first so that we test whether it waits for the
    # committee to be elected and connects to the leader.
    echo "Starting web3 gateway."
    run_gateway 1
    gateway_pid=$!

    wait ${gateway_pid}
}

run_test run_dummy_node
