download_oasis_binaries() {
	local out_dir=$1
	download_oasis_node $out_dir
	download_oasis_core_runtime_loader $out_dir
	download_keymanager_runtime $out_dir
	download_keymanager_runtime_sgx $out_dir
	download_runtime $out_dir
	download_runtime_sgx $out_dir
	download_gateway $out_dir
}

download_oasis_node() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core $OASIS_CORE_BRANCH "Build Go node" oasis-node $out_dir
	chmod +x $out_dir/oasis-node
}

download_oasis_net_runner() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core $OASIS_CORE_BRANCH "Build Go node" oasis-net-runner $out_dir
	chmod +x $out_dir/oasis-net-runner
}

download_oasis_core_runtime_loader() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core $OASIS_CORE_BRANCH "Build Rust runtime loader" oasis-core-runtime-loader $out_dir
	chmod +x $out_dir/oasis-core-runtime-loader
}

download_keymanager_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core $OASIS_CORE_BRANCH "Build key manager runtime" oasis-core-keymanager-runtime $out_dir
	chmod +x $out_dir/oasis-core-keymanager-runtime
}

download_keymanager_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core $OASIS_CORE_BRANCH "Build key manager runtime" oasis-core-keymanager-runtime.sgxs $out_dir
}

download_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime $RUNTIME_BRANCH "Build web3 gateway" gateway $out_dir
	chmod +x $out_dir/gateway
}

download_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime $RUNTIME_BRANCH "Build runtime" oasis-runtime $out_dir
	chmod +x $out_dir/oasis-runtime
}

download_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime $RUNTIME_BRANCH "Build runtime" oasis-runtime.sgxs $out_dir
}

download_oasis_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-gateway $OASIS_GATEWAY_BRANCH "Build" oasis-gateway $out_dir
	chmod +x $out_dir/oasis-gateway
}
