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

_assert_gateway_logs_not_contain() {
    set +ex
    pattern=$1
    msg=$2

    grep -q "${pattern}" ${EKIDEN_COMMITTEE_DIR}/gateway-*.log
    if [[ $? != "1" ]]; then
        echo -e "\e[31;1mTEST ASSERTION FAILED: ${msg}\e[0m"
        set -ex
        exit 1
    fi
    set -ex
}

assert_basic_gw_success() {
    assert_basic_success

    # Assert no panic on gateway.
    _assert_gateway_logs_not_contain "panicked at" "Panics detected during run."
}
