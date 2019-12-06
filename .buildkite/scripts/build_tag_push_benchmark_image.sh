#! /bin/bash

############################################################
# Simple wrapper script to call
# docker/benchmarking/docker_build_and_push.sh
# with the correct arguments.
#
# This script is intended to have buildkite
# specific things, like env vars and calling
# the buildkite-agent binary. Keeping this
# separate from the generic script that gets
# called allows us to use and test the generic
# scripts easily on a local dev box.
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

context=$1
runtime_base_tag=${2:-latest-testing}
target_tag=${3:-benchmarking-latest}

buildkite-agent artifact download "$context" .

docker/benchmarking/docker_build_and_push.sh \
  ${BUILDKITE_COMMIT} \
  ${target_tag} \
  "$context" \
  ${runtime_base_tag}