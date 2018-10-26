#!/bin/bash -e

# Runs the test suites for various popular dapps against our gateway.

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

    # Location for all the dapp repos
    mkdir -p /tmp/dapps
    cd /tmp/dapps

    run_dapps

    pkill -P $$
}

run_dapps() {
    #run_ens
    :
}

run_ens() {
    git clone https://github.com/oasislabs/ens.git
    cd ens
    git checkout ekiden
    npm install > /dev/null
    truffle test --network oasis_test & test_pid=$!

    wait $test_pid
    test_ret=$?
    if [ $test_ret -ne 0 ]; then
        echo "ens test suite failed"
        exit $test_ret
    fi
    cd ../
}

run_celer() {
    git clone https://github.com/oasislabs/cChannel-eth.git
    cd cChannel-eth
    git checkout ekiden
    npm install > /dev/null
    truffle compile
    truffle migrate --network oasis_test
    truffle test --network oasis_test & test_pid=$!

    wait $test_pid
    test_ret=$?
    if [ $test_ret -ne 0 ]; then
        echo "ens test suite failed"
        exit $test_ret
    fi
    cd ../
}

run_test
