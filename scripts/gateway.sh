#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ekiden \
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

run_compute_node() {
    local id=$1
    shift
    local extra_args=$*

    # Generate port number.
    let "port=id + 10000"

    echo "Starting compute node ${id} on port ${port}."

    ekiden-compute \
        --no-persist-identity \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent_${id} \
        --storage-multilayer-bottom-backend remote \
	--max-batch-timeout 100 \
	--entity-ethereum-address 0000000000000000000000000000000000000000 \
	--port ${port} \
        ${extra_args} \
        ${WORKDIR}/target/enclave/runtime-ethereum.so &> compute${id}.log &
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
    sleep 2

    # Advance epoch to elect a new committee.
    ekiden dummy set-epoch --epoch 1

    # Run the client. We run the client first so that we test whether it waits for the
    # committee to be elected and connects to the leader.
    echo "Starting web3 gateway."
    gateway/target/debug/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent-gateway \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --threads 100 &> gateway.log &
    gateway_pid=$!

    wait ${gateway_pid}

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
    wait || true
}

run_test run_dummy_node
