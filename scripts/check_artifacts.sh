#! /bin/bash


# Path to Oasis Core root.
OASIS_CORE_ROOT_PATH=${OASIS_CORE_ROOT_PATH:-${WORKDIR}}
# Path to the Oasis node.
OASIS_NODE=${OASIS_NODE:-${OASIS_CORE_ROOT_PATH}/go/oasis-node/oasis-node}
# Path to oasis-net-runner.
OASIS_NET_RUNNER=${OASIS_NET_RUNNER:-${OASIS_CORE_ROOT_PATH}/go/oasis-net-runner/oasis-net-runner}
# Path to the runtime loader.
OASIS_CORE_RUNTIME_LOADER=${OASIS_CORE_RUNTIME_LOADER:-${OASIS_CORE_ROOT_PATH}/target/default/debug/oasis-core-runtime-loader}

function check_executable() {
    if [[ ! -x ${!1} ]]; then
        echo "$1 not found at: '${!1}'. Make sure to set $1 or OASIS_CORE_ROOT_PATH env variable"
        exit 1
    fi
}

check_executable OASIS_NODE
check_executable OASIS_NET_RUNNER
check_executable OASIS_CORE_RUNTIME_LOADER
