#! /bin/bash

#########################################
# 1. Builds a new deployment image of
#    oasislabs/ci-runtime-ethereum
#    and tags it with the provided tag.
# 2. Push image to Docker Hub.
#########################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
docker_image_tag=$1

###############
# Optional args
###############
current_commit=$(git rev-parse --verify HEAD)
git_commit_sha=${2:-current_commit}

path_to_ssh_private_key=${3:-~/.ssh/id_rsa}

#################
# Local variables
#################
docker_image_name=oasislabs/ci-runtime-ethereum

# Hardcode a test tag name, just to be safe during development.
# TODO: remove before merging PR
docker_image_tag=ci-test-${docker_image_tag}

####################
# Build docker image
####################

set +x
# The docker command will contain the ssh private key
# in plain text and we don't want that getting into bash
# history, so we intentionally disable printing commands
# with set +x.
docker build \
  --rm \
  --force-rm \
  --build-arg SSH_PRIVATE_KEY="$(cat ${path_to_ssh_private_key})" \
  --build-arg EKIDEN_COMMIT_SHA="${git_commit_sha}" \
  --build-arg BUILD_IMAGE_TAG="${docker_image_tag}" \
  -t "${docker_image_name}:${docker_image_tag}" \
  docker/ci-runtime-ethereum
set -x

# Remove the intermediate docker images that contain
# the private SSH key.
docker rmi -f $(docker images -q --filter label=stage=builder)

# Push image to Docker Hub.
docker push ${docker_image_name}:${docker_image_tag}

