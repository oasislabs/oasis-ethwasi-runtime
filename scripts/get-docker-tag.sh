#!/bin/bash
##
# This is a build script to determine the value of the tag
# to use when versioning the docker image.
#
# Tag will be in format of:
#  | if git tag provided: <tag_name>-<date>
#  | otherwise:           <branch_name>-<date>
#
# The value of the tag will be echo'd to stdout
# so that the calling script can do something useful
# with it. For example, make it available elsewhere
# in the pipeline by saving it as an artifact/meta-data/etc.
#
# Usage:
# get-docker-tag.sh [git_branch] [git_tag]
##

# get-docker-tag $BUILDKITE_BRANCH $BUILDKITE_TAG

set -euo pipefail

git_branch_name=$1
git_tag_name=${2:-NO_TAG_PROVIDED}

# TODO possibly change to more human readable format:
#      YYYY-mm-dd-HH-MM-SS
timestamp=`date +%Y%m%d%H%M%S` # YYYYmmddHHMMSS

# Use the current git tag as a prefix if it is
# defined. If not, use "master" as a default.
if [ ${git_tag_name} = "NO_TAG_PROVIDED" ]; then
  prefix="${git_branch_name}"
else
  prefix=${git_tag_name}
fi

# Concat prefix and timestamp.
docker_image_tag=${prefix}-${timestamp}

# Echo the final tag value to stdout
# so that the calling script can do
# something useful with it.
echo "${docker_image_tag}"
