#!/bin/bash
##
# Triggers the ops-production build hook service
#
# Usage:
#   ./buildhook-notify.sh <deploy_target> <repository> <deploy_tag> <private_ops_revision> <secret_token>
#
# <deploy-target> - The environment to deploy to
# <repository> - The repository calling private-ops for deployment
# <tag> - The deployment tag to use
# <private_ops_revision> - The version of private ops to use for deployment
# <secret_token> - The circleci token to make api requests
##
set -euxo pipefail

deploy_target=$1
repository=$2
tag=$3
private_ops_revision=$4
secret_token=$5

curl -X POST \
     -H "Content-Type: application/json" \
     -d '{"revision": "'${private_ops_revision}'", "build_parameters": {"CIRCLE_JOB": "deploy-'${deploy_target}'", "DEPLOY_IMAGE_TAG": "'${tag}'", "DEPLOY_CALLER":"'${repository}'"}}' \
     https://circleci.com/api/v1.1/project/github/oasislabs/private-ops/tree/${private_ops_revision}?circle-token=${secret_token}
