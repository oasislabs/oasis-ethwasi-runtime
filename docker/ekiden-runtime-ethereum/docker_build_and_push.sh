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

###############
# Optional args
###############
path_to_ssh_private_key=${3:-"~/.ssh/id_rsa"}

#################
# Local variables
#################
docker_image_name=oasislabs/ekiden-runtime-ethereum

# Hardcode a test tag name, just to be safe during development.
# TODO: remove before merging PR
docker_image_tag=ci-test-${docker_image_tag}

####################################
# Build and publish the docker image
####################################

set +x
# The docker command will contain the ssh private key
# in plain text and we don't want that getting into bash
# history, so we intentionally disable printing commands
# with set +x.
docker build --rm --force-rm \
  --build-arg SSH_PRIVATE_KEY="$(cat ${path_to_ssh_private_key})" \
  --build-arg RUNTIME_ETHEREUM_COMMIT_SHA=${git_commit_sha} \
  --build-arg RUNTIME_ETHEREUM_BUILD_IMAGE_TAG=${docker_image_tag} \
  -t oasislabs/ekiden-runtime-ethereum:${docker_image_tag} \
  docker/ekiden-runtime-ethereum
set -x

docker push ${docker_image_name}:${docker_image_tag}

# Remove the intermediate docker images that contain
# the private SSH key
docker rmi -f $(docker images -q --filter label=stage=builder)
