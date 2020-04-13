#! /bin/bash

############################################################
# Simple wrapper script to call
# docker/deployment/build_context.sh
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

oasis_core_version=$1
output=$2

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

docker/deployment/build_context.sh "$oasis_core_version" "$output"
