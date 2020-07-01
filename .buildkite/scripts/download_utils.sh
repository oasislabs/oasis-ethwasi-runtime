download_oasis_binaries() {
	local out_dir=$1
	download_oasis_core_artifacts $out_dir
	download_keymanager_runtime $out_dir
	download_keymanager_runtime_sgx $out_dir
	download_runtime $out_dir
	download_runtime_sgx $out_dir
	download_gateway $out_dir
}

download_oasis_core_artifacts() {
	local out_dir=$1

	oasis_core_version=$(cat OASIS_CORE_VERSION)

	mkdir -p "${out_dir}/go/oasis-node"
	mkdir -p "${out_dir}/go/oasis-net-runner"
	mkdir -p "${out_dir}/target/debug/"

	curl -L -o /tmp/oasis_core_linux_amd64.tar.gz \
		"https://github.com/oasisprotocol/oasis-core/releases/download/v${oasis_core_version}/oasis_core_${oasis_core_version}_linux_amd64.tar.gz"
	tar -C /tmp/ -xzf /tmp/oasis_core_linux_amd64.tar.gz

	mv /tmp/oasis-node "${out_dir}/go/oasis-node/"
	mv /tmp/oasis-net-runner "${out_dir}/go/oasis-net-runner/"
	mv /tmp/oasis-core-runtime-loader "${out_dir}/target/debug/"
}

download_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime-ci $RUNTIME_BRANCH "Build web3 gateway" gateway $out_dir
	chmod +x $out_dir/gateway
}

download_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime-ci $RUNTIME_BRANCH "Build runtime" oasis-ethwasi-runtime $out_dir
	chmod +x $out_dir/oasis-ethwasi-runtime
}

download_keymanager_runtime() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core-ci $RUNTIME_BRANCH "Build key manager runtime" oasis-ethwasi-runtime-keymanager $out_dir
	chmod +x $out_dir/oasis-ethwasi-runtime-keymanager
}

download_keymanager_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-core-ci $RUNTIME_BRANCH "Build key manager runtime" oasis-ethwasi-runtime-keymanager.sgxs $out_dir
}

download_runtime_sgx() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-runtime-ci $RUNTIME_BRANCH "Build runtime" oasis-ethwasi-runtime.sgxs $out_dir
}

download_oasis_gateway() {
	local out_dir=$1
	.buildkite/scripts/download_artifact.sh oasis-gateway $OASIS_GATEWAY_BRANCH "Build" oasis-gateway $out_dir
	chmod +x $out_dir/oasis-gateway
}
