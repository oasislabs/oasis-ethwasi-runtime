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
if [ -n "${BUILD_PRODUCTION_GENESIS:-}" -a -n "${BUILD_BENCHMARKING:-}" ]; then
    echo "Cannot use BUILD_PRODUCTION_GENESIS and BUILD_BENCHMARKING options together!"
    exit 1
fi

if [ -n "${BUILD_PRODUCTION_GENESIS:-}" ]; then
    export RUNTIME_BUILD_EXTRA_ARGS='--features production-genesis'
    export GATEWAY_BUILD_EXTRA_ARGS='--features production-genesis'
fi

if [ -n "${BUILD_BENCHMARKING:-}" ]; then
    export RUNTIME_BUILD_EXTRA_ARGS='--features benchmarking'
    export GATEWAY_BUILD_EXTRA_ARGS='--features benchmarking'
fi

buildkite-agent artifact download "$platform_dir/*" .

docker/ekiden-runtime-ethereum/build_context.sh "$platform_dir" "$context"
