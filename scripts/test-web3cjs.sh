#!/bin/bash -e

# Runs the integration tests in the web3c.js repo against a gateway.

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    run_dummy_node_go_tm
    sleep 1
    run_compute_node 1
    sleep 1
    run_compute_node 2
    sleep 1
    run_gateway 1
    sleep 1

    # Advance epoch to elect a new commitee
    ${WORKDIR}/ekiden-node dummy set-epoch --epoch 1

    mkdir -p /tmp/testing

    cd /tmp/testing
    git clone https://github.com/oasislabs/web3c.js.git
    cd /tmp/testing/web3c.js

    npm install > /dev/null

    # Export the mnemonic and gateway url so they're available in the tests.
    export MNEMONIC="patient oppose cotton portion chair gentle jelly dice supply salmon blast priority"
    export GATEWAY="http://localhost:8545"

    echo "Running web3c.js tests against the gateway"
    npm run test:gateway & test_pid=$!

    wait $test_pid
    test_ret=$?
    if [ $test_ret -ne 0 ]; then
        echo "web3.js test failed"
    exit $test_ret
    fi

    pkill -P $$
}

run_test
