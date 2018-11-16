#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node_go_tm() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ${WORKDIR}/ekiden-node \
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
        &> dummy-go.log &
}

run_compute_node() {
    local id=$1
    shift
    local extra_args=$*

    local cache_dir=/tmp/ekiden-test-worker-cache-$id
    rm -rf ${cache_dir}

    # Generate port number.
    let "port=id + 10000"

    echo "Starting compute node ${id} on port ${port}."

    ekiden-compute \
        --worker-path $(which ekiden-worker) \
        --worker-cache-dir ${cache_dir} \
        --no-persist-identity \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent_${id} \
        --storage-multilayer-bottom-backend remote \
        --max-batch-timeout 100 \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
        --disable-key-manager \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target/enclave/runtime-ethereum.so &> compute${id}.log &
}

run_gateway() {
    local id=$1

    # Generate port numbers.
    let "http_port=id + 8544"
    let "ws_port=id + 8554"
    let "prometheus_port=id + 3000"

    echo "Starting web3 gateway ${id} on ports ${http_port} and ${ws_port}."
    target/debug/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent-gateway_${id} \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --http-port ${http_port} \
        --threads 100 \
        --ws-port ${ws_port} \
        --prometheus-metrics-addr 0.0.0.0:${prometheus_port} \
        --prometheus-mode pull &> gateway${id}.log &
}

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    # Start dummy node.
    # bash ${WORKDIR}/scripts/utils.sh run_dummy_node_go_tm
    run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    # bash ${WORKDIR}/scripts/utils.sh run_compute_node 1
    run_compute_node 1
    sleep 1
    # bash ${WORKDIR}/scripts/utils.sh run_compute_node 2
    run_compute_node 2

    # bash ${WORKDIR}/scripts/utils.sh run_gateway 1
    run_gateway 1
    sleep 3

    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

    echo "Installing RPC test dependencies."
    pushd ${WORKDIR}/tests/ > /dev/null
    git clone https://github.com/oasislabs/rpc-tests.git --branch ekiden
    pushd rpc-tests > /dev/null
    npm install > /dev/null
    echo "Running RPC tests."
    ./run_tests.sh

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test
