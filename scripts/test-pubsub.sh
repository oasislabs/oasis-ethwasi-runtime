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
        --prometheus-mode pull -v &> gateway${id}.log &
}

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    echo "Building contract."
    pushd ${WORKDIR}/tests/contracts/storage_contract > /dev/null
    ./build.sh
    popd > /dev/null

    # Start dummy node.
    run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    run_gateway 1
    sleep 10

    ${WORKDIR}/ekiden-node dummy set-epoch --epoch 1

    echo "Running truffle tests."
    pushd ${WORKDIR}/tests > /dev/null
    npm test > ${WORKDIR}/truffle.txt & truffle_pid=$!
    popd > /dev/null

    echo "Subscribing to pubsub."
    ${WORKDIR}/tests/web3js/test_pubsub.js &> pubsub.log

    PUBSUB=`grep 'transactionHash' pubsub.log` || exit 1

    # Check truffle test exit code
    wait $truffle_pid
    truffle_ret=$?
    if [ $truffle_ret -ne 0 ]; then
        echo "truffle test failed"
        exit $truffle_ret
    fi

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test
