#! /bin/bash

############################################################
# Simple wrapper script to call
# docker/ekiden-runtime-ethereum/build_context.sh
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

platform_dir=$1
context=$2
if [ -n "${BUILD_PRODUCTION_GENESIS:-}" ]; then
    export RUNTIME_BUILD_EXTRA_ARGS='--features production-genesis'
    export GATEWAY_BUILD_EXTRA_ARGS='--features production-genesis'
fi

buildkite-agent artifact download "$platform_dir/*" .

docker/ekiden-runtime-ethereum/build_context.sh "$platform_dir" "$context"
