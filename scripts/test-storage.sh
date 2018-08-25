#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node_default() {
    echo "Starting dummy node."

    ekiden-node-dummy \
        --random-beacon-backend dummy \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
        --time-source-notifier mockrpc \
        --storage-backend dummy \
        &> dummy.log &
}

run_compute_node() {
    local id=$1
    shift
    local extra_args=$*

    # Generate port number.
    let "port=id + 10000"

    echo "Starting compute node ${id} on port ${port}."

    ekiden-compute \
        --no-persist-identity \
        --batch-storage immediate_remote \
        --max-batch-timeout 100 \
        --time-source-notifier system \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target/enclave/runtime-ethereum.so &> compute${id}.log &
}

run_test() {
    local dummy_node_runner=$1

    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    # Start dummy node.
    $dummy_node_runner
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    # Advance epoch to elect a new committee.
    echo "Advancing epoch."
    sleep 2
    ekiden-node-dummy-controller set-epoch --epoch 1
    sleep 2

    echo "Starting web3 gateway."
    target/debug/gateway \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --threads 100 &> gateway.log &
    sleep 3


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

run_test run_dummy_node_default
