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
        --node-address unix:${EKIDEN_CLIENT_SOCKET} \
        --runtime-id 0000000000000000000000000000000000000000000000000000000000000000 \
        --http-port ${http_port} \
        --ws-port ${ws_port} 2>&1 | tee ${EKIDEN_COMMITTEE_DIR}/gateway-$id.log | sed "s/^/[gateway-${id}] /" &
}
