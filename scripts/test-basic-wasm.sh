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
        --disable-key-manager \
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
    pushd ${WORKDIR}/tests/contracts/basic_wasm_contract > /dev/null
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

    ${WORKDIR}/ekiden-node dummy set-epoch --epoch 1

    echo "Installing deploy_contract dependencies."
    pushd ${WORKDIR}/tests/deploy_contract > /dev/null
    npm install > /dev/null
    npm install > /dev/null # continue installing once secp256k1 fails to install
    echo "Deploying and calling contract."
    OUTPUT="$(./deploy_contract.js ${WORKDIR}/tests/contracts/basic_wasm_contract/target/basic_contract.wasm | tail -1)"
    echo "Contract address: $OUTPUT"
    OUTPUT="$(./call_contract.js --contract-address $OUTPUT | tail -1)"
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

run_test
