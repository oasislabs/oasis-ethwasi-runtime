#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node_default() {
    echo "Starting dummy node."

    ekiden-node-dummy \
        --time-source mockrpc \
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
	--max-batch-timeout 10 \
	--time-source-notifier system \
	--entity-ethereum-address 0000000000000000000000000000000000000000 \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target_benchmark/contract/runtime-evm.so &> compute${id}.log &
}

run_test() {
    local dummy_node_runner=$1

    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

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

    # Start genesis state injector.
    echo "Starting genesis state injector."
    ${WORKDIR}/genesis/target/release/genesis \
        --mr-enclave $(cat ${WORKDIR}/target_benchmark/contract/runtime-evm.mrenclave) \
	${WORKDIR}/genesis/state-999999.json &
    genesis_pid=$!

    # Wait on genesis.
    wait ${genesis_pid}

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
    wait || true
}

run_test run_dummy_node_default
