#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    # Start dummy node.
    bash ${WORKDIR}/scripts/utils.sh run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    bash ${WORKDIR}/scripts/utils.sh run_compute_node 1
    sleep 1
    bash ${WORKDIR}/scripts/utils.sh run_compute_node 2

    bash ${WORKDIR}/scripts/utils.sh run_gateway 1
    sleep 10

    echo "Building contract."
    rustup target add wasm32-unknown-unknown
    pushd ${WORKDIR}/tests/contracts/basic_wasm_contract > /dev/null
    ./build.sh
    popd > /dev/null

    echo "Installing deploy_contract dependencies."
    pushd ${WORKDIR}/tests/deploy_contract > /dev/null
    npm install > /dev/null
    npm install > /dev/null # continue installing once secp256k1 fails to install
    echo "Deploying and calling contract."
    OUTPUT="$(./deploy_contract.js ${WORKDIR}/tests/contracts/basic_wasm_contract/target/basic_wasm_contract.wasm | tail -1)"
    echo "Fetched: $OUTPUT"

    if [ "$OUTPUT" = "0x726573756c74" ]; then
        echo "Test passed."
    else
        echo "Incorrect output. Expected 0x726573756c74."
        exit 1
    fi

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test run_dummy_node_go_tm
