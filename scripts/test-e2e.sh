#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node_default() {
    echo "Starting dummy node."

    ekiden-node-dummy \
	--random-beacon-backend dummy \
	--entity-ethereum-address 0000000000000000000000000000000000000000 \
	--time-source-notifier mockrpc \
        --storage-backend dummy \
        &> dummy.log &
}

run_compute_node() {
    local id=$1
    shift
    local extra_args=$*

    # Generate port number.
    let "port=id + 10000"

    echo "Starting compute node ${id} on port ${port}."

    ekiden-compute \
        --no-persist-identity \
	--batch-storage immediate_remote \
	--max-batch-timeout 100 \
	--time-source-notifier system \
	--entity-ethereum-address 0000000000000000000000000000000000000000 \
	--port ${port} \
        ${extra_args} \
        ${WORKDIR}/target/contract/runtime-evm.so &> compute${id}.log &
}

run_test() {
    local dummy_node_runner=$1

    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    # Ensure jq is installed (used by test_origin.sh)
    apt-get install jq

    # Start dummy node.
    $dummy_node_runner
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    # Advance epoch to elect a new committee.
    echo "Advancing epoch."
    sleep 2
    ekiden-node-dummy-controller set-epoch --epoch 1
    sleep 2

    # Run the client. We run the client first so that we test whether it waits for the
    # committee to be elected and connects to the leader.
    echo "Starting web3 gateway."
    gateway/target/debug/gateway \
        --mr-enclave $(cat $WORKDIR/target/contract/runtime-evm.mrenclave) \
        --threads 100 &> gateway.log &
    sleep 2

    # Run origin test script
    echo "Running origin test script."
    pushd ${WORKDIR}/scripts/ > /dev/null
    . test_origin.sh
    popd > /dev/null

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test run_dummy_node_default
