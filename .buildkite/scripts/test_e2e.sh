#!/bin/bash

############################################################
# This script tests the Oasis runtime project.
#
# Usage:
# test_e2e.sh [-w <workdir>] [-t <test-name>]
############################################################

# Defaults.
WORKDIR=$(pwd)
TEST_FILTER=""

# We need a directory in the workdir so that Buildkite can fetch artifacts.
if [[ ! -z ${BUILDKITE} ]]; then
    TEST_BASE_DIR="${WORKDIR}/e2e"
    mkdir -p ${TEST_BASE_DIR}
fi

# Temporary test base directory.
TEST_BASE_DIR=$(realpath ${TEST_BASE_DIR:-$(mktemp -d --tmpdir oasis-runtime-e2e-XXXXXXXXXX)})

# Path to Oasis Core root.
OASIS_CORE_ROOT_PATH=${OASIS_CORE_ROOT_PATH:-${WORKDIR}}
# Path to the Oasis node.
OASIS_NODE=${OASIS_NODE:-${OASIS_CORE_ROOT_PATH}/go/oasis-node/oasis-node}
# Path to oasis-net-runner.
OASIS_NET_RUNNER=${OASIS_NET_RUNNER:-${OASIS_CORE_ROOT_PATH}/go/oasis-net-runner/oasis-net-runner}
# Path to the runtime loader.
OASIS_CORE_RUNTIME_LOADER=${OASIS_CORE_RUNTIME_LOADER:-${OASIS_CORE_ROOT_PATH}/target/debug/oasis-core-runtime-loader}
# Path to keymanager binary.
OASIS_CORE_KM_BINARY=${OASIS_CORE_KM_BINARY:-${WORKDIR}/target/debug/oasis-runtime-keymanager}
# Path to runtime binary.
RUNTIME_BINARY=${RUNTIME_BINARY:-${WORKDIR}/target/debug/oasis-runtime}
# Path to runtime genesis state.
RUNTIME_GENESIS=${RUNTIME_GENESIS:-${WORKDIR}/resources/genesis/oasis_genesis_testing.json}
# Path to web3 gateway.
RUNTIME_GATEWAY=${RUNTIME_GATEWAY:-${WORKDIR}/target/debug/gateway}

#########################
# Process test arguments.
#########################
while getopts 'f:t:' arg
do
    case ${arg} in
        w) WORKDIR=${OPTARG};;
        t) TEST_FILTER=${OPTARG};;
        *)
            echo "Usage: $0 [-w <workdir>] [-t <test-name>]"
            exit 1
    esac
done

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source .buildkite/scripts/common.sh
source .buildkite/rust/common.sh

# Ensure NVM is loaded when present.
nvm_script="${NVM_DIR:-${HOME}/.nvm}/nvm.sh"
[ -s "${nvm_script}" ] && source "${nvm_script}"

###################
# Test definitions.
###################
# Global test counter used for parallelizing jobs.
E2E_TEST_COUNTER=0

# Run a specific test scenario.
#
# Required named arguments:
#
#   name           - unique test name
#   scenario       - function that will start the network
#
# Scenario function:
#
# The scenario function defines what will be executed during the test.
#
run_test() {
    # Required arguments.
    local name scenario
    # Optional arguments with default values.
    local pre_init_hook=""
    # Load named arguments that override defaults.
    local "${@}"

    # Check if we should run this test.
    if [[ "${TEST_FILTER:-}" == "" ]]; then
        local test_index=$E2E_TEST_COUNTER
        let E2E_TEST_COUNTER+=1 1

        if [[ -n ${BUILDKITE_PARALLEL_JOB+x} ]]; then
            let test_index%=BUILDKITE_PARALLEL_JOB_COUNT 1

            if [[ $BUILDKITE_PARALLEL_JOB != $test_index ]]; then
                echo "Skipping test '${name}' (assigned to different parallel build)."
                return
            fi
        fi
    elif [[ "${TEST_FILTER}" != "${name}" ]]; then
        return
    fi

    echo -e "\n\e[36;7;1mRUNNING TEST:\e[27m ${name}\e[0m\n"

    if [[ "${pre_init_hook}" != "" ]]; then
        $pre_init_hook
    fi

    # Run scenario.
    $scenario
}

scenario_basic() {
    # Run the network.
    echo "Starting the test network."
    ${OASIS_NET_RUNNER} \
        --fixture.default.node.binary ${OASIS_NODE} \
        --fixture.default.runtime.binary ${RUNTIME_BINARY} \
        --fixture.default.runtime.loader ${OASIS_CORE_RUNTIME_LOADER} \
        --fixture.default.runtime.genesis_state ${RUNTIME_GENESIS} \
        --fixture.default.keymanager.binary ${OASIS_CORE_KM_BINARY} \
        --fixture.default.epochtime_mock \
        --basedir.no_temp_dir \
        --basedir ${TEST_BASE_DIR} &

    local client_socket="${TEST_BASE_DIR}/net-runner/network/client-0/internal.sock"

    # Wait for the validator and keymanager nodes to be registered.
    echo "Waiting for the validator and keymanager to be registered."
    ${OASIS_NODE} debug control wait-nodes \
        --address unix:${client_socket} \
        --nodes 2 \
        --wait

    # Advance epoch.
    echo "Advancing epoch."
    ${OASIS_NODE} debug control set-epoch \
        --address unix:${client_socket} \
        --epoch 1

    # Wait for all nodes to be registered.
    echo "Waiting for all nodes to be registered."
    ${OASIS_NODE} debug control wait-nodes \
        --address unix:${client_socket} \
        --nodes 6 \
        --wait

    # Advance epoch.
    echo "Advancing epoch."
    ${OASIS_NODE} debug control set-epoch \
        --address unix:${client_socket} \
        --epoch 2

    # Start the gateway.
    echo "Starting the web3 gateway."
    ${RUNTIME_GATEWAY} \
        --node-address unix:${client_socket} \
        --runtime-id 8000000000000000000000000000000000000000000000000000000000000000 \
        --http-port 8545 \
        --ws-port 8555 2>&1 | tee ${TEST_BASE_DIR}/gateway.log | sed "s/^/[gateway] /" &

    sleep 3
}

###########
# RPC tests
###########
install_rpc_tests() {
    local rpc_tests_branch=${RPC_TESTS_BRANCH:-ekiden}

    echo "Installing RPC test dependencies."
    pushd ${WORKDIR}/tests
        if [ ! -d rpc-tests ]; then
            git clone https://github.com/oasislabs/rpc-tests.git --branch ${rpc_tests_branch} --depth 1

            pushd rpc-tests
                npm install > /dev/null
            popd
        fi
    popd
}

scenario_rpc_tests() {
    scenario_basic $*

    echo "Running RPC tests."
    pushd ${WORKDIR}/tests/rpc-tests
        ./run_tests.sh 2>&1 | tee ${TEST_BASE_DIR}/tests-rpc-tests.log
    popd
}

#################################
# Tests from e2e-tests repository
#################################
install_e2e_tests() {
    local e2e_tests_branch=${E2E_TESTS_BRANCH:-master}

    echo "Installing E2E tests from e2e-tests repository."
    pushd ${WORKDIR}/tests
        if [ ! -d e2e-tests ]; then
            git clone https://github.com/oasislabs/e2e-tests.git -b ${e2e_tests_branch} --depth 1
            pushd e2e-tests
                npm install > /dev/null
                # Needed to install and build oasis-client within e2e-tests.
                npm install -g lerna
                npm install -g yarn
                ./scripts/oasis-client.sh
                # If the Buildkite access token is available, download pre-compiled contracts
                # from the e2e-tests pipeline.
                if [ "${BUILDKITE:-}" == "true" ]; then
                    echo "Downloading compiled contracts from the e2e-tests pipeline."
                    # Solidity contracts.
                    ${WORKDIR}/.buildkite/scripts/download_artifact.sh \
                        e2e-tests \
                        ${e2e_tests_branch} \
                        "Lint and Compile Contracts" \
                        build.zip \
                        "$(pwd)"
                    unzip build.zip
                    rm build.zip
                    # Oasis services.
                    ${WORKDIR}/.buildkite/scripts/download_artifact.sh \
                        e2e-tests \
                        ${e2e_tests_branch} \
                        "Lint and Compile Contracts" \
                        services.zip \
                        "$(pwd)"
                    rm -rf services
                    unzip services.zip
                    rm services.zip
                else
                    # Ensure no special compiler flags are in effect.
                    unset RUSTFLAGS
                fi
            popd
        fi
    popd
}

scenario_e2e_tests() {
    scenario_basic $*

    echo "Starting the oasis-gateway"
    ./go/oasis-gateway/oasis-gateway \
        --config.path configs/developer-gateway/testing.toml \
        --bind_public.max_body_bytes 16777216 \
        --bind_public.http_write_timeout_ms 100000 &

    echo "Running E2E tests from e2e-tests repository."
    pushd ${WORKDIR}/tests/e2e-tests
        # Re-export parallelism parameters so that they can be read by the e2e-tests.
        export E2E_PARALLELISM=${BUILDKITE_PARALLEL_JOB_COUNT:-""}
        export E2E_PARALLELISM_BUCKET=${BUILDKITE_PARALLEL_JOB:-""}
        # Define the environment variables that are required for the e2e tests.
        export HTTPS_PROVIDER_URL="http://localhost:8545"
        export WS_PROVIDER_URL="ws://localhost:8555"
        export MNEMONIC="patient oppose cotton portion chair gentle jelly dice supply salmon blast priority"
        export OASIS_CLIENT_SK="533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c"
        export DEVELOPER_GATEWAY_URL="http://localhost:1234"
        # Cleanup persisted keys.
        rm -rf .oasis
        npm run test:development 2>&1 | tee ${TEST_BASE_DIR}/tests-e2e-tests.log
    popd
}

# RPC test.
run_test \
    pre_init_hook=install_rpc_tests \
    scenario=scenario_rpc_tests \
    name="rpc-tests"

# E2E tests from e2e-tests repository.
run_test \
    pre_init_hook=install_e2e_tests \
    scenario=scenario_e2e_tests \
    name="e2e-tests"
