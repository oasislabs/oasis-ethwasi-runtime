#!/bin/bash

oasis_core_root_path=$1
oasis_core_src_path=$2
runtime_root_path=$3
runtime_src_path=$4

set -euo pipefail

# Sanity check the source paths.
if [ ! -d "${oasis_core_src_path}" ]; then
    echo "ERROR: Invalid arguments specified."
    echo "       Invoke as: make symlink-artifacts OASIS_CORE_SRC_PATH=path/to/ekiden"
    exit 1
fi

# Symlink an artifact from the source directory to the root directory.
symlink_artifact() {
    local src_path=$1
    local binary_path=$2
    local root_path=$3
    local skip_sanity_check=${4:-"0"}

    local artifact_src_path="$(realpath "${src_path}/${binary_path}")"
    local artifact_dst_path="${root_path}/${binary_path}"

    # Sanity check the source artifact.
    if [[ "${skip_sanity_check}" != "1" && ! -f "${artifact_src_path}" ]]; then
        echo "ERROR: Artifact '${binary_path}' does not exist in specified path ${src_path}."
        echo "       Maybe you need to run: make -C \"${src_path}\""
        exit 1
    fi

    mkdir -p "$(dirname "${artifact_dst_path}")"
    ln -sf "${artifact_src_path}" "${artifact_dst_path}"
}

# Symlink all Oasis Core build artifacts.
symlink_artifact ${oasis_core_src_path} go/oasis-node/oasis-node ${oasis_core_root_path}
symlink_artifact ${oasis_core_src_path} go/oasis-net-runner/oasis-net-runner ${oasis_core_root_path}

# For Rust, symlink against the CARGO_TARGET_DIR instead of OASIS_CORE_SRC_PATH, if set.
set +u
if [ -n "${CARGO_TARGET_DIR}" ]; then
    oasis_core_src_path=$(dirname ${CARGO_TARGET_DIR})
    runtime_src_path=$(dirname ${CARGO_TARGET_DIR})
fi
set -u

symlink_artifact $oasis_core_src_path target/debug/oasis-core-runtime-loader  $oasis_core_root_path
symlink_artifact $oasis_core_src_path target/debug/oasis-core-keymanager-runtime $oasis_core_root_path
symlink_artifact $oasis_core_src_path target/x86_64-fortanix-unknown-sgx/debug/oasis-core-keymanager-runtime.sgxs $oasis_core_root_path

# Symlink the runtime artifacts.
symlink_artifact $runtime_src_path target/debug/oasis-runtime $runtime_root_path 1
