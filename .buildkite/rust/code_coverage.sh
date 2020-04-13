#!/bin/bash

#################################################
# This script uses Tarpaulin to calculate test
# coverage in the code base.
#
# Usage:
# code_coverage.sh [path_to_coveralls_api_token]
#
# path_to_coveralls_api_token - Absolute or relative
#     path to a file that contains the coveralls.io
#     API token. Defaults to "~/.coveralls/api_token".
#################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

source .buildkite/rust/common.sh

###############
# Optional args
###############
path_to_coveralls_api_token=${1:-~/.coveralls/oasis_runtime_api_token}

############
# Local vars
############
set +x
coveralls_api_token=$(cat ${path_to_coveralls_api_token})
set -x

#################################################
# Add github public key to known_hosts.
# This is required because some test scripts
# run `npm install` and at least one dependency
# has its own dependencies that pull from
# GitHub and the /root/.gitconfig file transforms
# https to ssh when pulling from GitHub.
#################################################
mkdir ~/.ssh/
ssh-keyscan rsa github.com >> ~/.ssh/known_hosts

# We need to use a separate target dir for tarpaulin as it otherwise clears
# the build cache.
export CARGO_TARGET_DIR=/var/tmp/coverage_target

# Possible workaround for #694
# https://github.com/xd009642/tarpaulin/issues/35
export RAYON_NUM_THREADS=1

# Name the current commit so Tarpaulin can detect it correctly.
git checkout -B ${BUILDKITE_BRANCH}

# Calculate coverage.
set +x
cargo tarpaulin \
  --packages oasis-runtime \
  --packages oasis-runtime-common \
  --packages web3-gateway \
  --packages oasis-runtime-keymanager \
  --exclude-files .e2e* \
  --exclude-files *generated* \
  --exclude-files genesis* \
  --exclude-files node_modules* \
  --ignore-tests \
  --out Xml \
  --coveralls ${coveralls_api_token} \
  --features test \
  -v
set -x

# Error if coverage results file does not exist.
# Workaround for `cargo tarpaulin` ignoring errors.
[ -f cobertura.xml ]
