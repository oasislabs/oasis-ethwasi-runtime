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
	--max-batch-timeout 100 \
	--max-batch-size 50 \
        --batch-storage immediate_remote \
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
        --mr-enclave $(cat target_benchmark/contract/runtime-evm.mrenclave) \
	${WORKDIR}/genesis/state-9999.json &> genesis.log &
    genesis_pid=$!

    # Wait on genesis.
    wait ${genesis_pid}

    # Run the client.
    echo "Starting web3 gateway."
    pushd ${WORKDIR}/client/ > /dev/null
    target/release/web3-client \
        --mr-enclave $(cat ${WORKDIR}/target_benchmark/contract/runtime-evm.mrenclave) \
        --threads 100 &> ${WORKDIR}/client.log &
    popd > /dev/null
    client_pid=$!
    sleep 5

    # Run transaction playback.
    echo "Starting transaction playback."
    ${WORKDIR}/playback/target/release/playback \
	--transactions 10000 \
	--threads 100 \
	${WORKDIR}/playback/blocks-10000-1000000.bin &> playback.log &
    playback_pid=$!

    # Wait on playback.
    wait ${playback_pid}

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
    wait || true
}

run_test run_dummy_node_default
