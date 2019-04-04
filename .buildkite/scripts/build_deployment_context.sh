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
export RUNTIME_BUILD_EXTRA_ARGS='--cargo-addendum feature.production-genesis.addendum -- --features production-genesis'
export GATEWAY_BUILD_EXTRA_ARGS='--features production-genesis'

buildkite-agent artifact download "$platform_dir/*" .

docker/ekiden-runtime-ethereum/build_context.sh "$platform_dir" "$context"
