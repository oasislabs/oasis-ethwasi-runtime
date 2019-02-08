# apt-get install jq

WORKDIR=${1:-$(pwd)}

OASIS_HOME_DIR="/tmp/oasis"
OASIS_ARTIFACTS_DIR="${OASIS_HOME_DIR}/artifacts"
EKIDEN_BRANCH="master"
RUNTIME_BRANCH="master"

if [ ! -d "$OASIS_ARTIFACTS_DIR" ]; then
	mkdir -p $OASIS_ARTIFACTS_DIR
	export BUILDKITE_ACCESS_TOKEN="e6dc7081e8629fe309040995d3ca0de11c9d0a96"
	source .buildkite/scripts/download_utils.sh

	download_ekiden_node $OASIS_ARTIFACTS_DIR
	download_ekiden_worker $OASIS_ARTIFACTS_DIR
	download_keymanager_node $OASIS_ARTIFACTS_DIR
	download_keymanager_enclave $OASIS_ARTIFACTS_DIR
	download_keymanager_mrenclave $OASIS_ARTIFACTS_DIR
	download_runtime_enclave $OASIS_ARTIFACTS_DIR
	download_runtime_mrenclave $OASIS_ARTIFACTS_DIR
fi

export EKIDEN_NODE=$OASIS_ARTIFACTS_DIR/ekiden-node
export EKIDEN_WORKER=$OASIS_ARTIFACTS_DIR/ekiden-worker
export KM_ENCLAVE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-trusted.so
export KM_MRENCLAVE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-trusted.mrenclave
export KM_NODE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-node
export GATEWAY=$OASIS_ARTIFACTS_DIR/gateway
export RUNTIME_ENCLAVE=$OASIS_ARTIFACTS_DIR/runtime-ethereum.so
export RUNTIME_MRENCLAVE=$OASIS_ARTIFACTS_DIR/runtime-ethereum.mrenclave

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

# Start keymanager node.
run_keymanager_node
sleep 1

TEST_BASE_DIR=$(mktemp -d --tmpdir ekiden-e2e-XXXXXXXXXX)
export EKIDEN_VALIDATOR_SOCKET=${TEST_BASE_DIR}/committee-data-1/internal.sock

# Start the gateway.
echo "Starting web3 gateway."
run_gateway 1

# Start validator committee.
run_backend_tendermint_committee
sleep 1

# Start compute nodes.
run_compute_committee > compute.txt
sleep 1

# Advance epoch to elect a new committee.
set_epoch 1

wait
