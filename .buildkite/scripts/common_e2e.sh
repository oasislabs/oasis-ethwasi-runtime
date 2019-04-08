# Path to runtime-ethereum gateway.
RUNTIME_GATEWAY=${RUNTIME_GATEWAY:-${WORKDIR}/target/debug/gateway}

# Run a runtime-ethereum gateway node.
#
# Arguments:
#   id - node identifier (default: 1)
run_gateway() {
    local id=${1:-1}

    # Generate port numbers.
    let http_port=id+8544
    let ws_port=id+8554
    let prometheus_port=id+3000

    ${RUNTIME_GATEWAY} \
        --node-address unix:${EKIDEN_VALIDATOR_SOCKET} \
        --runtime-id 0000000000000000000000000000000000000000000000000000000000000000 \
        --http-port ${http_port} \
        --threads 100 \
        --ws-port ${ws_port} 2>&1 | tee ${EKIDEN_COMMITTEE_DIR}/gateway-$id.log | sed "s/^/[gateway-${id}] /" &
}

run_backend_tendermint_committee_custom() {
    run_backend_tendermint_committee \
        replica_group_size=3
}

run_no_client() {
    :
}

scenario_basic() {
    local runtime=$1

    # Initialize compute nodes.
    run_compute_node 1 ${runtime}
    run_compute_node 2 ${runtime}
    run_compute_node 3 ${runtime}
    run_compute_node 4 ${runtime}

    # Wait for all compute nodes to start.
    wait_compute_nodes 4

    # Advance epoch to elect a new committee.
    set_epoch 1

    # Initialize gateway.
    run_gateway 1
    sleep 3
}
