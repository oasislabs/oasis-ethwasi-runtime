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
	cp /ekiden/bin/ekiden-node $out_dir/ekiden
	chmod +x $out_dir/ekiden
}

download_ekiden_runtime_loader() {
	local out_dir=$1
	cp /ekiden/bin/ekiden-node $out_dir/ekiden-runtime-loader
	chmod +x $out_dir/ekiden-runtime-loader
}

download_keymanager_runtime() {
	local out_dir=$1
	cp /ekiden/lib/ekiden-keymanager-runtime $out_dir/ekiden-keymanager-runtime
	chmod +x $out_dir/ekiden-keymanager-runtime
}

download_keymanager_runtime_sgx() {
	local out_dir=$1
	cp /ekiden/lib/ekiden-keymanager-runtime-sgxs $out_dir
}

download_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime-ci $RUNTIME_BRANCH "Build web3 gateway" gateway $out_dir
	chmod +x $out_dir/gateway
}

download_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime-ci $RUNTIME_BRANCH "Build runtime" runtime-ethereum $out_dir
	chmod +x $out_dir/runtime-ethereum
}

download_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime-ci $RUNTIME_BRANCH "Build runtime" runtime-ethereum.sgxs $out_dir
}

download_developer_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh developer-gateway $DEVELOPER_GATEWAY_BRANCH "Build" developer-gateway $out_dir
	chmod +x $out_dir/developer-gateway
}
