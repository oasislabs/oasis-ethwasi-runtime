#! /bin/bash

#############################################
# Simple wrapper script to call
# buildhook-notify.sh
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

####################
# Required arguments
####################
environment_to_deploy_to=$1

#################
# Local variables
#################
deployment_image_tag=$(buildkite-agent meta-data \
                       get \
                       "deployment_image_tag"
                     )

repository=oasislabs/runtime-ethereum
private_ops_revision=master

circleci_secret_token=$(cat ~/.circleci/private_ops_api_token)

########################################
# Trigger deploy to staging via CircleCI
########################################
scripts/buildhook-notify.sh \
  ${environment_to_deploy_to} \
  ${repository} \
  ${deployment_image_tag} \
  ${private_ops_revision} \
  ${circleci_secret_token}
