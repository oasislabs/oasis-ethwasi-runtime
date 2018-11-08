#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
    # Start dummy node.
    run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    run_gateway 1
    sleep 10

    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

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
}

run_test
