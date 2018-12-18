#!/bin/bash -e

# Runs the integration tests in the web3c.js repo against a gateway.

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
    run_dummy_node_go_tm
    sleep 1
    run_keymanager_node
    sleep 1
    run_compute_committee
    sleep 1
    run_gateway 1
    sleep 1

    # Advance epoch to elect a new commitee
    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

    mkdir -p /tmp/testing

    cd /tmp/testing
    if [ ! -d web3c.js ]; then
      git clone \
        https://github.com/oasislabs/web3c.js.git \
        --depth 1
    fi

    cd web3c.js

    git pull

    npm install > /dev/null

    # Export the mnemonic and gateway url so they're available in the tests.
    export MNEMONIC="patient oppose cotton portion chair gentle jelly dice supply salmon blast priority"
    export GATEWAY="http://localhost:8545"

    echo "Running web3c.js tests against the gateway"
    npm run test:gateway
}

run_test
