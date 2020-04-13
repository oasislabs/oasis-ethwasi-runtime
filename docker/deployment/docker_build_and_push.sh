#! /bin/bash

#########################################
# 1. Builds a new deployment image of
#    oasislabs/ekiden-runtime-ethereum
#    and tags it with the provided tag.
# 2. Push deployment image to Docker Hub.
#########################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
git_commit_sha=$1
docker_image_tag=$2
oasis_core_version=$3
context=$4

###############
# Optional args
###############
base_image_tag=${4:-latest}

#################
# Local variables
#################
docker_image_name=oasislabs/oasis-runtime

####################################
# Build and publish the docker image
####################################

set +x
# The docker command will contain the ssh private key
# in plain text and we don't want that getting into bash
# history, so we intentionally disable printing commands
# with set +x.
docker build --pull --rm --force-rm \
  --build-arg OASIS_RUNTIME_COMMIT_SHA=${git_commit_sha} \
  --build-arg OASIS_RUNTIME_BUILD_IMAGE_TAG=${docker_image_tag} \
  --build-arg OASIS_CORE_VERSION=${oasis_core_version} \
  -t oasislabs/${docker_image_name}:${docker_image_tag} \
  --file=docker/deployment/Dockerfile \
  - <"$context"
set -x

docker push ${docker_image_name}:${docker_image_tag}
