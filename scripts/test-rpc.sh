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
    # Start keymanager node.
    run_keymanager_node
    sleep 1
    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    run_gateway 1
    sleep 3

    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

    echo "Installing RPC test dependencies."
    cd ${WORKDIR}/tests
    if [ ! -d rpc-tests ]; then
      git clone https://github.com/oasislabs/rpc-tests.git --branch ekiden
    fi

    cd rpc-tests
    git pull
    npm install > /dev/null
    
    echo "Running RPC tests."
    ./run_tests.sh
}

run_test
