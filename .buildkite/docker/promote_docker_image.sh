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
existing_docker_image_name_and_tag=$1
new_docker_image_name_and_tag=$2

#############################
# Pull, retag, and push image
#############################

docker pull ${existing_docker_image_name_and_tag}

docker tag \
  ${existing_docker_image_name_and_tag} \
  ${new_docker_image_name_and_tag}

docker push ${new_docker_image_name_and_tag}
