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
context=$3

###############
# Optional args
###############
#base_image_tag=${4:-latest}
# hardcoded kryha base image
base_image_tag=a686c03603e60385ea6c2c26fe50b33eb1ce55a0-testing

#################
# Local variables
#################
docker_image_name=oasislabs/ekiden-runtime-ethereum

####################################
# Build and publish the docker image
####################################

set +x
# The docker command will contain the ssh private key
# in plain text and we don't want that getting into bash
# history, so we intentionally disable printing commands
# with set +x.
docker build --pull --rm --force-rm \
  --build-arg RUNTIME_ETHEREUM_COMMIT_SHA=${git_commit_sha} \
  --build-arg RUNTIME_ETHEREUM_BUILD_IMAGE_TAG=${docker_image_tag} \
  --build-arg OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG=${base_image_tag} \
  -t oasislabs/ekiden-runtime-ethereum:${docker_image_tag} \
  --file=docker/ekiden-runtime-ethereum/Dockerfile \
  - <"$context"
set -x

docker push ${docker_image_name}:${docker_image_tag}
