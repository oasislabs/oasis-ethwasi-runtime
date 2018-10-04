#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node_go_tm() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ${WORKDIR}/ekiden-node \
        --log.level debug \
        --grpc.port 42261 \
        --epochtime.backend tendermint \
        --epochtime.tendermint.interval 90 \
        --beacon.backend tendermint \
        --storage.backend memory \
        --scheduler.backend trivial \
        --registry.backend tendermint \
        --roothash.backend tendermint \
        --datadir ${datadir} \
        &> dummy-go.log &
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
        --storage-backend remote \
        --max-batch-timeout 100 \
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

    echo "Starting web3 gateway."
    target/debug/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent-gateway \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --threads 100 &> gateway.log &
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
