#!/bin/bash

set -euo pipefail

E2E_SCRIPTS_DIR=.e2e

# Check if test scripts already exist and do nothing if they do.
if [ -e ${E2E_SCRIPTS_DIR}/ekiden_common_e2e.sh ]; then
    echo "Found existing ekiden E2E scripts in ${E2E_SCRIPTS_DIR}. Not updating."
    exit 0
fi

# Detect current ekiden branch.
# TODO: Make this more robust.
: ${EKIDEN_REPO:=$(grep 'ekiden-runtime =' Cargo.toml | grep -Eo 'git = "(.+)"' | cut -d '"' -f 2)}
: ${EKIDEN_BRANCH:=$(grep 'ekiden-runtime =' Cargo.toml | grep -Eo 'branch = "(.+)"' | cut -d '"' -f 2)}
if [ "$EKIDEN_BRANCH" == "" ]; then
    echo "ERROR: Unable to determine the ekiden branch."
    exit 1
fi

echo "Updating ekiden E2E scripts (repo \"${EKIDEN_REPO}\", branch \"${EKIDEN_BRANCH}\")..."

# Download Ekiden test scripts.
rm -rf ${E2E_SCRIPTS_DIR}
mkdir -p ${E2E_SCRIPTS_DIR}
pushd ${E2E_SCRIPTS_DIR}
    git clone ${EKIDEN_REPO} -b ${EKIDEN_BRANCH} --depth 1
    ln -s ekiden/.buildkite/scripts/common_e2e.sh ekiden_common_e2e.sh
popd

echo "Ekiden E2E scripts updated."
