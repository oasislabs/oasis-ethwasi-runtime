#!/bin/bash

# Runs the test suites for dapps against our gateway.
# CLI args: "augur", "celer" or "ens".

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

WORKDIR=$(pwd)

source scripts/utils.sh $WORKDIR

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
	source ./scripts/oasis.sh false
	run_test_network
    # Location for all the dapp repos
    mkdir -p /tmp/dapps
    cd /tmp/dapps

    run_dapp $1

    # Dump gateway metrics
    curl -v http://localhost:3001/metrics
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
            run_ens
            ;;
    esac
}

run_ens() {
    if [ ! -d ens ]; then
      git clone \
        https://github.com/oasislabs/ens.git \
        --depth 1 \
        --branch ekiden
    fi

    cd ens
    git pull

    npm install > /dev/null
    npm run test
}

run_celer() {
    if [ ! -d cChannel-eth ]; then
      git clone \
        https://github.com/oasislabs/cChannel-eth.git \
        --depth 1 \
        --branch ekiden
    fi

    cd cChannel-eth
    git pull

    npm install > /dev/null
    npm run test
}

run_augur() {
    apt-get update
    apt-get install -y python3-pip
    pip3 install virtualenv
    npm install npx

    if [ ! -d augur-core ]; then
      git clone \
        https://github.com/oasislabs/augur-core.git \
        --depth 1 \
        --branch ekiden
    fi

    cd augur-core
    git pull

    npm install > /dev/null

    pip3 install -r requirements.txt

    export OASIS_PRIVATE_KEY=c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179
    export PATH=$PATH:$(pwd)/bin

    npm run build
    npm run test:integration
}

run_test $1
