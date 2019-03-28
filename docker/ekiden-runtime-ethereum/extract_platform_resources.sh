#!/bin/bash

# Extract some resources from the oasislabs/testnet image of a given tag.
# See also scripts/oasis.sh, which obtains resources directly form Buildkite.

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

###############
# Required args
###############
dst_dir=$1
base_image_tag=${2:-latest}

mkdir -p "$dst_dir"

container=$(docker create "oasislabs/testnet:$base_image_tag")
trap "docker rm $container" EXIT

docker cp "$container:/ekiden/lib/ekiden-keymanager-runtime.sgxs" "$dst_dir"
