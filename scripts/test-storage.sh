#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

run_dummy_node_go_tm() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ${WORKDIR}/ekiden-node \
        --log.level debug \
        --grpc.port 42261 \
        --epochtime.backend tendermint_mock \
        --beacon.backend insecure \
        --storage.backend memory \
        --scheduler.backend trivial \
        --registry.backend tendermint \
        --roothash.backend tendermint \
        --tendermint.consensus.timeout_commit 250ms \
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
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent_${id} \
        --storage-multilayer-bottom-backend remote \
        --max-batch-timeout 100 \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target/enclave/runtime-ethereum.so &> compute${id}.log &
}

run_gateway() {
    local id=$1

    # Generate port numbers.
    let "http_port=id + 8544"
    let "ws_port=id + 8554"
    let "prometheus_port=id + 3000"

    echo "Starting web3 gateway ${id} on ports ${http_port} and ${ws_port}."
    target/debug/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent-gateway_${id} \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --http-port ${http_port} \
        --threads 100 \
        --ws-port ${ws_port} \
        --prometheus-metrics-addr 0.0.0.0:${prometheus_port} \
        --prometheus-mode pull &> gateway${id}.log &
}

run_test() {
    # Ensure cleanup on exit.
    trap 'kill -- -0' EXIT

    echo "Building contract."
    pushd ${WORKDIR}/tests/contracts/storage_contract > /dev/null
    ./build.sh
    popd > /dev/null

    # Start dummy node.
    # bash ${WORKDIR}/scripts/utils.sh run_dummy_node_go_tm
    run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    # bash ${WORKDIR}/scripts/utils.sh run_compute_node 1
    run_compute_node 1
    sleep 1
    # bash ${WORKDIR}/scripts/utils.sh run_compute_node 2
    run_compute_node 2

    # bash ${WORKDIR}/scripts/utils.sh run_gateway 1
    run_gateway 1
    sleep 10


    echo "Uploading bytes to storage."
    curl -X POST -H 'Content-Type: application/json' --data '{"jsonrpc":"2.0","method":"oasis_storeBytes","params":[[1, 2, 3, 4, 5], 9223372036854775807],"id":"1"}' localhost:8545 > /dev/null

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
