# apt-get install jq

WORKDIR=${1:-$(pwd)}

OASIS_HOME_DIR="/tmp/oasis"
OASIS_ARTIFACTS_DIR="${OASIS_HOME_DIR}/artifacts"
EKIDEN_BRANCH="master"
RUNTIME_BRANCH="master"

# Download all the artifacts that we need to run a local network,
# if they don't already exist.
if [ ! -d "$OASIS_ARTIFACTS_DIR" ]; then
	mkdir -p $OASIS_ARTIFACTS_DIR
	export BUILDKITE_ACCESS_TOKEN=${BUILDKITE_ACCESS_TOKEN:-""}
	source .buildkite/scripts/download_utils.sh

	download_ekiden_node $OASIS_ARTIFACTS_DIR
	download_ekiden_worker $OASIS_ARTIFACTS_DIR
	download_keymanager_node $OASIS_ARTIFACTS_DIR
	download_keymanager_enclave $OASIS_ARTIFACTS_DIR
	download_keymanager_mrenclave $OASIS_ARTIFACTS_DIR
	download_runtime_enclave $OASIS_ARTIFACTS_DIR
	download_runtime_mrenclave $OASIS_ARTIFACTS_DIR
	download_gateway $OASIS_ARTIFACTS_DIR
fi

source scripts/utils.sh

# Define these so that we override the paths define in scripts.utils.sh.
export EKIDEN_NODE=$OASIS_ARTIFACTS_DIR/ekiden-node
export EKIDEN_WORKER=$OASIS_ARTIFACTS_DIR/ekiden-worker
export KM_ENCLAVE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-trusted.so
export KM_MRENCLAVE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-trusted.mrenclave
export KM_NODE=$OASIS_ARTIFACTS_DIR/ekiden-keymanager-node
export GATEWAY=$OASIS_ARTIFACTS_DIR/gateway
export RUNTIME_ENCLAVE=$OASIS_ARTIFACTS_DIR/runtime-ethereum.so
export RUNTIME_MRENCLAVE=$OASIS_ARTIFACTS_DIR/runtime-ethereum.mrenclave

trap 'cleanup' EXIT
run_test_network
wait
