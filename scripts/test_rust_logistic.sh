#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    echo "Building contract."
    pushd ${WORKDIR}/tests/contracts/rust-logistic-contract > /dev/null
    ./build.sh
    popd > /dev/null

    # Start dummy node.
    run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    run_gateway 1
    sleep 10

    ${WORKDIR}/ekiden-node dummy set-epoch --epoch 1

    echo "Installing deploy_contract dependencies."
    pushd ${WORKDIR}/tests/deploy_contract > /dev/null
    npm install > /dev/null
    npm install > /dev/null # continue installing once secp256k1 fails to install
    echo "Deploying and calling contract."
    OUTPUT="$(./deploy_contract.js --gas-limit 0x1e84800 --gas-price 0x3b9aca00 ${WORKDIR}/tests/contracts/rust-logistic-contract/target/rust_logistic_contract.wasm | tail -1)"
    echo "Contract address: $OUTPUT"
    OUTPUT="$(./call_contract.js $OUTPUT | tail -1)"
    echo "Fetched: $OUTPUT"

    if [ "$OUTPUT" = "0x4d61746368696e6720636c617373657320697320313030" ]; then
        echo "Test passed."
    else
        echo "Incorrect output. Expected 0x4d61746368696e6720636c617373657320697320313030."
        exit 1
    fi

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test
