#!/bin/bash

set -euo pipefail

WORKDIR=$(pwd)

# For automatic cleanup on exit.
source .buildkite/scripts/common.sh
source .e2e/ekiden_common_e2e.sh
source .buildkite/scripts/common_e2e.sh

scenario_run_gateway() {
	scenario_basic runtime-ethereum
	sleep infinity
}

run_test \
    pre_init_hook=run_no_client \
    scenario=scenario_run_gateway \
    name="gateway.sh" \
    backend_runner=run_backend_tendermint_committee_custom \
    runtime=runtime-ethereum \
    client_runner=run_no_client
