#!/bin/bash -ex

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
        --prometheus-mode pull &> gateway${id}.log &
}

run_test() {
    local dummy_node_runner=$1

    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    # Run the gateway. We start the gateway first so that we test 1) whether the
    # snapshot manager can recover after initially failing to connect to the
    # root hash stream, and 2) whether the gateway waits for the committee to be
    # elected and connects to the leader.
    run_gateway 1
    run_gateway 2
    sleep 3

    # Start dummy node.
    $dummy_node_runner
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    # Advance epoch to elect a new committee.
    sleep 3
    ${WORKDIR}/ekiden-node dummy set-epoch --epoch 1

    # Run truffle tests against gateway 1 (in background)
    echo "Running truffle tests."
    pushd ${WORKDIR}/tests/ > /dev/null
    npm test > ${WORKDIR}/truffle.txt & truffle_pid=$!
    popd > /dev/null

    echo "Subscribing to log notifications on web3js."
    ${WORKDIR}/tests/web3js/test_pubsub.js &> pubsub.log &

    # Subscribe to logs from gateway 2, and check that we get a log result
    echo "Subscribing to log notifications."
    RESULT=`wscat --connect localhost:8556 -w 120 -x "{\"id\": 1, \"jsonrpc\":\"2.0\", \"method\": \"eth_subscribe\", \"params\": [\"logs\", { \"fromBlock\": \"latest\", \"toBlock\": \"latest\" }]}" | jq -e .params.result.transactionHash`
    echo $RESULT

    PUBSUB=`grep 'transactionHash' pubsub.log` || exit 1

    # Check truffle test exit code
    wait $truffle_pid
    truffle_ret=$?
    if [ $truffle_ret -ne 0 ]; then
        echo "truffle test failed"
	exit $truffle_ret
    fi

    # Dump the metrics.
    curl -v http://localhost:3001/metrics
    curl -v http://localhost:3002/metrics

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test run_dummy_node_go_tm
