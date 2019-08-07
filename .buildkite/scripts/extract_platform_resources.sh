#!/bin/bash

############################################################
# Simple wrapper script to call
# docker/ekiden-runtime-ethereum/extract_platform_resources.sh
############################################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

dst_dir=$1
tag_suffix=${BASE_VARIANT:+-$BASE_VARIANT}

docker/ekiden-runtime-ethereum/extract_platform_resources.sh "$dst_dir" "latest$tag_suffix"
