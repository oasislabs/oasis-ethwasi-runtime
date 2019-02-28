#! /bin/bash

############################################################
# Simple wrapper script to call
# docker/ekiden-runtime-ethereum/docker_build_and_push.sh
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

deployment_image_tag=$(buildkite-agent meta-data \
                       get \
                       "deployment_image_tag"
                     )
tag_suffix=${DEPLOYMENT_VARIANT:+-$DEPLOYMENT_VARIANT}
export RUNTIME_BUILD_EXTRA_ARGS='--cargo-addendum feature.production-genesis.addendum -- --features production-genesis'
export GATEWAY_BUILD_EXTRA_ARGS='--features production-genesis'

docker/ekiden-runtime-ethereum/docker_build_and_push.sh \
  ${BUILDKITE_COMMIT} \
  ${deployment_image_tag}${tag_suffix} \
  "" \
  latest${tag_suffix}
