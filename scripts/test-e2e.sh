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
    # Spin up the local testnet.
    run_test_network

    # Run truffle tests against gateway 1 (in background).
    echo "Running truffle tests."
    pushd /e2e-tests > /dev/null
    # Define the environment variables that are required for the e2e tests.
    export HTTPS_PROVIDER_URL="http://localhost:8545"
    export WS_PROVIDER_URL="ws://localhost:8555"
    export MNEMONIC="patient oppose cotton portion chair gentle jelly dice supply salmon blast priority"
    # See https://github.com/oasislabs/ekiden/blob/master/key-manager/dummy/enclave/src/lib.rs
    export KEY_MANAGER_PUBLIC_KEY="0x9d41a874b80e39a40c9644e964f0e4f967100c91654bfd7666435fe906af060f"
    npm run test:development & truffle_pid=$!
    popd > /dev/null

    # Wait for truffle tests, ensure they did not fail.
    wait $truffle_pid

    # Dump the metrics from both gateways.
    curl -v http://localhost:3001/metrics
    curl -v http://localhost:3002/metrics
}

run_test
