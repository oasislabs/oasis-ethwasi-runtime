#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
    local dummy_node_runner=$1

    run_keymanager_node
    sleep 1

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
    run_compute_committee
    sleep 3

    # Advance epoch to elect a new committee.
    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

    # Run truffle tests against gateway 1 (in background)
    echo "Running truffle tests."
    pushd ${WORKDIR}/tests/ > /dev/null
    npm test > ${WORKDIR}/truffle.txt & truffle_pid=$!
    popd > /dev/null

    # Subscribe to logs from gateway 2, and check that we get a log result
    echo "Subscribing to log notifications."
    RESULT=`wscat --connect localhost:8556 -w 120 -x "{\"id\": 1, \"jsonrpc\":\"2.0\", \"method\": \"eth_subscribe\", \"params\": [\"logs\", { \"fromBlock\": \"latest\", \"toBlock\": \"latest\" }]}" | jq -e .params.result.transactionHash` || exit 1

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
}

run_test run_dummy_node_go_tm
