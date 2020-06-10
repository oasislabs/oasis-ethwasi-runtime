#!/bin/bash

############################################################
# This script builds a runtime.
#
# Usage:
# build_runtime.sh <src_dir>
#
# src_dir - Absolute or relative path to the directory
#           containing the source code.
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
src_dir=$1
if [ ! -d $src_dir ]; then
  echo "ERROR: Invalid source directory specified (${src_dir})."
  exit 1
fi
shift

# Runtime variant (elf, sgxs).
variant=${RUNTIME_VARIANT:-elf}

source .buildkite/rust/common.sh

#######################################################
# Update the PATH to respect $CARGO_INSTALL_ROOT.
# This allows 'cargo install' to reuse binaries
# from previous installs as long as the correct
# host directory is mounted on the docker container.
# Huge speed improvements during local dev and testing.
#######################################################
set +u
export PATH=$CARGO_INSTALL_ROOT/bin/:$PATH
set -u

#################################################################
# Ensure we have ekiden-tools installed, needed to build enclaves
#################################################################
if [ ! -x ${CARGO_INSTALL_ROOT}/bin/cargo-elf2sgxs ]; then
    cargo install \
        --force \
        --git https://github.com/oasisprotocol/oasis-core \
        --branch $OASIS_CORE_BRANCH \
        --debug \
        oasis-core-tools
fi

###################
# Build the runtime
###################
pushd $src_dir
    case $variant in
        elf)
            # Build non-SGX runtime.
            OASIS_UNSAFE_SKIP_KM_POLICY="1" cargo build --locked
            ;;
        sgxs)
            unset OASIS_UNSAFE_SKIP_KM_POLICY
            # Build SGX runtime.
            cargo build --locked --target x86_64-fortanix-unknown-sgx
            cargo elf2sgxs
            ;;
    esac

    ######################################
    # Apply the rust code formatting rules
    ######################################
    cargo fmt -- --check
popd
