#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
    # Start keymanager node.
    run_keymanager_node
    sleep 1

    # Since we run the gateway first, we need the socket path to connect to. This
    # should be synced with how 'run_backend_tendermint_committee' generates the
    # socket path.
    export EKIDEN_VALIDATOR_SOCKET=${TEST_BASE_DIR}/committee-data-1/internal.sock

    # Run the gateway. We start the gateway first so that we test 1) whether the
    # snapshot manager can recover after initially failing to connect to the
    # root hash stream, and 2) whether the gateway waits for the committee to be
    # elected and connects to the leader.
    run_gateway 1
    run_gateway 2
    sleep 3

    # Start validator committee.
    run_backend_tendermint_committee
    sleep 1

    # Start compute nodes.
    run_compute_committee
    sleep 1

    # Advance epoch to elect a new committee.
    set_epoch 1

    # Run truffle tests against gateway 1 (in background).
    echo "Running truffle tests."
    pushd /e2e-tests > /dev/null
    # Ensure the CARGO_TARGET_DIR is not set so that oasis-compile can generate the
    # correct rust contract artifacts. Can remove this once the following is
    # addressed: https://github.com/oasislabs/oasis-compile/issues/44
    unset CARGO_TARGET_DIR
    # Define the environment variables that are required for the e2e tests.
    export HTTPS_PROVIDER_URL="http://localhost:8545"
    export WS_PROVIDER_URL="ws://localhost:8555"
    export MNEMONIC="patient oppose cotton portion chair gentle jelly dice supply salmon blast priority"
    npm run test:development & truffle_pid=$!
    popd > /dev/null

    # Wait for truffle tests, ensure they did not fail.
    wait $truffle_pid

    # Dump the metrics from both gateways.
    curl -v http://localhost:3001/metrics
    curl -v http://localhost:3002/metrics
}

run_test
