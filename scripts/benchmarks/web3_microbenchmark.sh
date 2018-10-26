#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ekiden \
        --log.level error \
        --grpc.port 42261 \
        --epochtime.backend tendermint \
        --tendermint.consensus.timeout_commit 250ms \
        --epochtime.tendermint.interval 240 \
        --beacon.backend tendermint \
        --storage.backend leveldb \
        --scheduler.backend trivial \
        --registry.backend tendermint \
        --roothash.backend tendermint \
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
        --max-batch-size 50 \
        --max-batch-timeout 100 \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
	--disable-key-manager \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target_benchmark/enclave/runtime-ethereum.so &> compute${id}.log &
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

    # Run the client. We run the client first so that we test whether it waits for the
    # committee to be elected and connects to the leader.
    echo "Starting web3 gateway."
    gateway/target/release/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent-gateway \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target_benchmark/enclave/runtime-ethereum.mrenclave) \
        --threads 100 &> gateway.log &
    sleep 2

    # Start benchmark.
    echo "Starting benchmark."
    benchmark \
        --benchmarks.concurrency 100 \
        --benchmarks transfer,eth_blockNumber,net_version,eth_getBlockByNumber \
        --log.level INFO
    benchmark_pid=$!

    # Wait on the benchmark and check its exit status.
    wait ${benchmark_pid}

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
    wait || true
}

run_test run_dummy_node
