download_oasis_binaries() {
	local out_dir=$1
	download_ekiden_node $out_dir
	download_ekiden_runtime_loader $out_dir
	download_keymanager_runtime $out_dir
	download_keymanager_runtime_sgx $out_dir
	download_runtime $out_dir
	download_runtime_sgx $out_dir
	download_gateway $out_dir
}

download_ekiden_node() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build Go node" ekiden $out_dir
	chmod +x $out_dir/ekiden
}

download_ekiden_net_runner() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build Go node" ekiden-net-runner $out_dir
	chmod +x $out_dir/ekiden-net-runner
}

download_ekiden_runtime_loader() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build Rust runtime loader" ekiden-runtime-loader $out_dir
	chmod +x $out_dir/ekiden-runtime-loader
}

download_keymanager_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build key manager runtime" ekiden-keymanager-runtime $out_dir
	chmod +x $out_dir/ekiden-keymanager-runtime
}

download_keymanager_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build key manager runtime" ekiden-keymanager-runtime.sgxs $out_dir
}

download_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh runtime-ethereum $RUNTIME_BRANCH "Build web3 gateway" gateway $out_dir
	chmod +x $out_dir/gateway
}

download_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh runtime-ethereum $RUNTIME_BRANCH "Build runtime" runtime-ethereum $out_dir
	chmod +x $out_dir/runtime-ethereum
}

download_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh runtime-ethereum $RUNTIME_BRANCH "Build runtime" runtime-ethereum.sgxs $out_dir
}

download_developer_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh developer-gateway $DEVELOPER_GATEWAY_BRANCH "Build" developer-gateway $out_dir
	chmod +x $out_dir/developer-gateway
}
