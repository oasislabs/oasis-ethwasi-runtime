#!/bin/bash -e

# Runs the test suites for various popular dapps against our gateway.

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Parent account responsible for funding the various dapp accounts.
OASIS_MNEMONIC='patient oppose cotton portion chair gentle jelly dice supply salmon blast priority'
# Export these so that they can be used in their respective tests.
export ENS_MNEMONIC='patient oppose cotton portion chair gentle jelly dice supply salmon blast ens'
export CELER_MNEMONIC='patient oppose cotton portion chair gentle jelly dice supply salmon blast celer'

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    run_testnet
    fund_accounts
    run_dapps

    pkill -P $$
}

run_testnet() {
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
}

fund_accounts() {
    cd ${WORKDIR}/scripts/dapps
    npm install
    num_accounts=4
    amount=10000000000000000000
    node ${WORKDIR}/scripts/dapps/fundAccounts.js "$OASIS_MNEMONIC" "$ENS_MNEMONIC" $num_accounts $amount
    node ${WORKDIR}/scripts/dapps/fundAccounts.js "$OASIS_MNEMONIC" "$CELER_MNEMONIC" $num_accounts $amount
    cd ../../
}

run_dapps() {
    # Location for all the dapp repos
    mkdir -p /tmp/dapps
    cd /tmp/dapps
    run_ens > test_ens.txt & ens_pid=$!
    run_celer > test_celer.txt & celer_pid=$!
    wait $ens_pid
    wait $celer_pid
}

run_ens() {
    git clone https://github.com/oasislabs/ens.git
    cd ens
    git checkout armani/feature/ekiden
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

test_wait() {
    local test_pid=$1
    wait $test_pid
    test_ret=$?
    if [ $test_ret -ne 0 ]; then
        echo "test suite failed"
        exit $test_ret
    fi
}

run_test
