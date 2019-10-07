#!/bin/bash

set -euo pipefail

ekiden_root_path=$1
ekiden_src_path=$2
runtime_root_path=$3
runtime_src_path=$4

# Sanity check the source path.
if [ ! -d "${runtime_src_path}" ]; then
    echo "ERROR: Invalid arguments specified'."
    echo "       Invoke as: make symlink-artifacts EKIDEN_SRC_PATH=path/to/ekiden"
    exit 1
fi

# Symlink an artifact from the source directory to the root directory.
symlink_artifact() {
    local src_path=$1
    local binary_path=$2
    local root_path=$3

    local artifact_src_path="$(realpath "${src_path}/${binary_path}")"
    local artifact_dst_path="${root_path}/${binary_path}"

    # Sanity check the source artifact.
    if [ ! -f "${artifact_src_path}" ]; then
        echo "ERROR: Artifact '${binary_path}' does not exist in specified path ${src_path}."
        echo "       Maybe you need to run: make -C \"${src_path}\""
        exit 1
    fi

    mkdir -p "$(dirname "${artifact_dst_path}")"
    ln -sf "${artifact_src_path}" "${artifact_dst_path}"
}

# Symlink all ekiden build artifacts.
symlink_artifact ${ekiden_src_path} go/ekiden/ekiden ${ekiden_root_path}
symlink_artifact ${ekiden_src_path} go/ekiden-net-runner/ekiden-net-runner ${ekiden_root_path}

# For Rust, symlink against the CARGO_TARGET_DIR instead of EKIDEN_SRC_PATH, if set.
set +u
if [ -n "${CARGO_TARGET_DIR}" ]; then
    ekiden_src_path=$(dirname ${CARGO_TARGET_DIR})
    runtime_src_path=$(dirname ${CARGO_TARGET_DIR})
fi
set -u

symlink_artifact $ekiden_src_path target/debug/ekiden-runtime-loader  $ekiden_root_path
symlink_artifact $ekiden_src_path target/debug/ekiden-keymanager-runtime $ekiden_root_path
symlink_artifact $ekiden_src_path target/x86_64-fortanix-unknown-sgx/debug/ekiden-keymanager-runtime.sgxs $ekiden_root_path

# Symlink the runtime artifacts.
symlink_artifact $runtime_src_path target/debug/runtime-ethereum $runtime_root_path
