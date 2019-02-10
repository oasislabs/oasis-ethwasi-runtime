download_ekiden_node() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build Go node" ekiden $out_dir
	mv $out_dir/ekiden $out_dir/ekiden-node
	chmod +x $out_dir/ekiden-node
}

download_ekiden_worker() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build Rust worker, compute node and key manager node" ekiden-worker $out_dir
	chmod +x $out_dir/ekiden-worker
}

download_keymanager_node() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build Rust worker, compute node and key manager node" ekiden-keymanager-node $out_dir
	chmod +x $out_dir/ekiden-keymanager-node
}

download_keymanager_enclave() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build key manager enclave" ekiden-keymanager-trusted.so $out_dir
}

download_keymanager_mrenclave() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh ekiden $EKIDEN_BRANCH "Build key manager enclave" ekiden-keymanager-trusted.mrenclave $out_dir
}

download_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh runtime-ethereum $RUNTIME_BRANCH "Build web3 gateway" gateway $out_dir
	chmod +x $out_dir/gateway
}

download_runtime_enclave() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh runtime-ethereum $RUNTIME_BRANCH "Build runtime" runtime-ethereum.so $out_dir
}

download_runtime_mrenclave() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh runtime-ethereum $RUNTIME_BRANCH "Build runtime" runtime-ethereum.mrenclave $out_dir
}
