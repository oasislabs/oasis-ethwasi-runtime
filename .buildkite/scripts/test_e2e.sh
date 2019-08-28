#!/bin/bash

############################################################
# This script tests the Ekiden project.
#
# Usage:
# test_e2e.sh [-w <workdir>] [-t <test-name>]
############################################################

# Defaults.
WORKDIR=$(pwd)
TEST_FILTER=""

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
source .e2e/ekiden_common_e2e.sh
source .buildkite/scripts/common_e2e.sh
source .buildkite/rust/common.sh

# Ensure NVM is loaded when present.
nvm_script="${NVM_DIR:-${HOME}/.nvm}/nvm.sh"
[ -s "${nvm_script}" ] && source "${nvm_script}"

###################
# Test definitions.
###################
run_backend_tendermint_committee_custom() {
    run_backend_tendermint_committee \
        epochtime_backend=tendermint_mock \
        replica_group_size=3 \
        runtime_genesis=${WORKDIR}/resources/genesis/ekiden_genesis_testing.json
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

    # Initialize storage nodes.
    run_storage_node 1
    run_storage_node 2

    # Wait for all compute nodes to start.
    wait_nodes 6 # 4 + 2 storage

    # Advance epoch to elect a new committee.
    set_epoch 1

    # Initialize gateway.
    run_gateway 1
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
        ./run_tests.sh 2>&1 | tee ${EKIDEN_COMMITTEE_DIR}/tests-rpc-tests.log
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
                    # Mantle contracts.
                    ${WORKDIR}/.buildkite/scripts/download_artifact.sh \
                        e2e-tests \
                        ${e2e_tests_branch} \
                        "Lint and Compile Contracts" \
                        mantle.zip \
                        "$(pwd)"
                    rm -rf mantle
                    unzip mantle.zip
                    rm mantle.zip
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

    echo "Starting the developer-gateway"
    ./go/developer-gateway/developer-gateway \
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
        npm run test:development 2>&1 | tee ${EKIDEN_COMMITTEE_DIR}/tests-e2e-tests.log
    popd
}

#############
# Test suite.
#
# Arguments:
#    backend_name - name of the backend to use in test name
#    backend_runner - function that will prepare and run the backend services
#############
test_suite() {
    local backend_name=$1
    local backend_runner=$2

    # RPC test.
    run_test \
        pre_init_hook=install_rpc_tests \
        scenario=scenario_rpc_tests \
        name="e2e-${backend_name}-rpc-tests" \
        backend_runner=$backend_runner \
        runtime=runtime-ethereum \
        client_runner=run_no_client

    # E2E tests from e2e-tests repository.
    run_test \
        pre_init_hook=install_e2e_tests \
        scenario=scenario_e2e_tests \
        name="e2e-${backend_name}-e2e-tests" \
        backend_runner=$backend_runner \
        runtime=runtime-ethereum \
        client_runner=run_no_client
}

##########################################
# Multiple validators tendermint backends.
##########################################
test_suite tm-committee run_backend_tendermint_committee_custom
