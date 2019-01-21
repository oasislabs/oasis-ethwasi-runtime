#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
    # Start validator committee.
    run_backend_tendermint_committee
    sleep 1
    # Start keymanager node.
    run_keymanager_node
    sleep 1
    # Start compute nodes.
    run_compute_committee
    sleep 1
    run_gateway 1
    sleep 3

    set_epoch 1

    echo "Running truffle tests."
    pushd ${WORKDIR}/tests > /dev/null
    # Ensure the CARGO_TARGET_DIR is not set so that oasis-compile can generate the
    # correct rust contract artifacts. Can remove this once the following is
    # addressed: https://github.com/oasislabs/oasis-compile/issues/44
    unset CARGO_TARGET_DIR
    npm test & truffle_pid=$!
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
