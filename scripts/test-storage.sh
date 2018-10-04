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


    echo "Uploading bytes to storage."
    curl -X POST -H 'Content-Type: application/json' --data '{"jsonrpc":"2.0","method":"oasis_storeBytes","params":[[1, 2, 3, 4, 5], 9223372036854775807],"id":"1"}' localhost:8545 > /dev/null

    echo "Building contract."
    rustup target add wasm32-unknown-unknown
    cargo install --git https://github.com/oasislabs/wasm-utils --branch ekiden
    pushd ${WORKDIR}/tests/contracts/storage_contract > /dev/null
    ./build.sh
    popd > /dev/null

    echo "Installing deploy_contract dependencies."
    pushd ${WORKDIR}/tests/deploy_contract > /dev/null
    npm install > /dev/null
    npm install > /dev/null # continue installing once secp256k1 fails to install
    echo "Deploying and calling contract."
    OUTPUT="$(./deploy_contract.js ${WORKDIR}/tests/contracts/storage_contract/target/storage_contract.wasm | tail -1)"
    echo "Fetched: $OUTPUT"

    if [ "$OUTPUT" = "0x0102030405" ]; then
        echo "Test passed."
    else
        echo "Incorrect output. Expected 0x0102030405."
        exit 1
    fi

    # Cleanup.
    echo "Cleaning up."
    pkill -P $$
}

run_test
