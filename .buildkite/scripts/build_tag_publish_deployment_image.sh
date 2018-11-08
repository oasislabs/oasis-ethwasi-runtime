#! /bin/bash

#############################################
# Simple wrapper script to call
# build_tag_publish_deployment_image.sh
# with the correct arguments.
# 
# This script is intended to have buildkite
# specific things, like env vars and calling
# the buildkite-agent binary. Keeping this
# separate from the generic script that gets
# called allows us to use and test the generic
# scripts easily on a local dev box.
##############################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

deployment_image_tag=$(buildkite-agent meta-data \
                       get \
                       "deployment_image_tag"
                     )

.buildkite/docker/build_tag_publish_deployment_image.sh \
  ${BUILDKITE_COMMIT} \
  ${deployment_image_tag}
