#!/bin/bash

set -euo pipefail

root_path=$1
src_path=$2

# Sanity check the source path.
if [ ! -d "${src_path}" ]; then
    echo "ERROR: Invalid EKIDEN_SRC_PATH specified: '${src_path}'."
    echo "       Invoke as: make symlink-ekiden EKIDEN_SRC_PATH=path/to/ekiden"
    exit 1
fi

# Symlink an artifact from the source directory to the root directory.
symlink_artifact() {
    local path=$1

    local artifact_src_path="$(realpath "${src_path}/${path}")"
    local artifact_dst_path="${root_path}/${path}"

    # Sanity check the source artifact.
    if [ ! -f "${artifact_src_path}" ]; then
        echo "ERROR: Artifact '${path}' does not exist in specified EKIDEN_SRC_PATH."
        echo "       Maybe you need to run: make -C \"${src_path}\""
        exit 1
    fi

    mkdir -p "$(dirname "${artifact_dst_path}")"
    ln -sf "${artifact_src_path}" "${artifact_dst_path}"
}

# Symlink all ekiden build artifacts.
symlink_artifact go/ekiden/ekiden
symlink_artifact target/debug/ekiden-runtime-loader
symlink_artifact target/debug/ekiden-keymanager-runtime
symlink_artifact target/x86_64-fortanix-unknown-sgx/debug/ekiden-keymanager-runtime.sgxs
