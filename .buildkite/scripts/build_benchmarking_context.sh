#! /bin/bash

############################################################
# Simple wrapper script to call
# docker/benchmarking/build_context.sh
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

context=$1

docker/benchmarking/build_context.sh "$context"
