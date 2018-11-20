#!/bin/bash -e

# Runs the test suites for dapps against our gateway.
# CLI args: "augur", "celer" or "ens".

source scripts/utils.sh

WORKDIR=$(pwd)

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
    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

    # Location for all the dapp repos
    mkdir -p /tmp/dapps
    cd /tmp/dapps

    run_dapp $1

    pkill -P $$
}

run_dapp() {
    case "$1" in
        "augur")
            run_augur
            ;;
        "celer")
            run_celer
            ;;
        "ens")
            #run_ens
            :
            ;;
    esac
}

run_ens() {
    git clone https://github.com/oasislabs/ens.git
    cd ens
    git checkout ekiden
    npm install > /dev/null
    truffle test --network oasis_test & test_pid=$!
    test_wait $test_pid
    cd ../
}

run_celer() {
    git clone https://github.com/oasislabs/cChannel-eth.git
    cd cChannel-eth
    git checkout ekiden
    npm install > /dev/null
    truffle compile > /dev/null
    truffle migrate --network oasis_test
    truffle test --network oasis_test & test_pid=$!
    test_wait $test_pid
    cd ../
}

run_augur() {
    apt-get update
    apt-get install -y python3-pip
    pip3 install virtualenv
    npm install npx

    git clone https://github.com/oasislabs/augur-core.git
    cd augur-core
    git checkout ekiden
    npm install > /dev/null
    pip3 install -r requirements.txt

    export OASIS_PRIVATE_KEY=c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179
    export PATH=$PATH:$(pwd)/bin

    npm run build
    npm run test:integration
}

test_wait() {
    local test_pid=$1
    wait $test_pid
    test_ret=$?
    if [ $test_ret -ne 0 ]; then
        echo "ens test suite failed"
        exit $test_ret
    fi
}

run_test $1
