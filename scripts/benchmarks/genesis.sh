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

run_dummy_node_storage_dynamodb() {
    echo "Starting dummy node."

    ekiden-node-dummy \
        --time-source-notifier mockrpc \
        --random-beacon-backend dummy \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
        --storage-backend dynamodb \
        --storage-dynamodb-region us-east-1 \
        --storage-dynamodb-table-name test \
        &> dummy.log &
}

run_compute_node_default() {
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
        --batch-storage immediate_remote \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target_benchmark/enclave/runtime-ethereum.so &> compute${id}.log &
}

run_compute_node_storage_multilayer() {
    local id=$1
    shift
    local extra_args=$*

    local db_dir=/tmp/ekiden-test-storage-multilayer-sled-$id
    # Generate port number.
    let "port=id + 10000"

    echo "Starting compute node ${id} on port ${port}."

    ekiden-compute \
        --no-persist-identity \
        --max-batch-size 50 \
	--max-batch-timeout 10 \
        --time-source-notifier system \
	--entity-ethereum-address 0000000000000000000000000000000000000000 \
        --batch-storage multilayer \
        --storage-multilayer-sled-storage-base "$db_dir" \
        --storage-multilayer-aws-region us-east-1 \
        --storage-multilayer-aws-table-name test \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target_benchmark/enclave/runtime-ethereum.so &> compute${id}.log &
}

run_test() {
    local dummy_node_runner=$1
    local compute_node_runner=$2

    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    # Start dummy node.
    $dummy_node_runner
    sleep 1

    # Start compute nodes.
    $compute_node_runner 1
    sleep 1
    $compute_node_runner 2

    # Advance epoch to elect a new committee.
    echo "Advancing epoch."
    sleep 2
    ekiden-node-dummy-controller set-epoch --epoch 1
    sleep 2

    # Start genesis state injector.
    echo "Starting genesis state injector."
    ${WORKDIR}/genesis/target/release/genesis \
        --mr-enclave $(cat ${WORKDIR}/target_benchmark/enclave/runtime-ethereum.mrenclave) \
	${WORKDIR}/genesis/state-999999.json &
    genesis_pid=$!

    # Wait on genesis.
    wait ${genesis_pid}

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
    wait || true
}

#run_test run_dummy_node_storage_dynamodb run_compute_node_storage_multilayer
run_test run_dummy_node_default run_compute_node_default
