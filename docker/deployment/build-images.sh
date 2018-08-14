#!/bin/bash -e

runtime_ethereum_commit_sha=${CIRCLE_SHA1:-unknown}
base_docker_image_tag=${BASE_DOCKER_IMAGE_TAG:-latest}
ekiden_image=${EKIDEN_DOCKER_IMAGE:-ekiden/development:0.2.0}
base_dir=$( cd "$( dirname "${BASH_SOURCE[0]}" )/../.." && pwd )

cd ${base_dir}

if [ -n "$BUILD_IMAGES_NO_ENTER" ]; then
    ./docker/deployment/build-images-inner.sh
elif [ -z "$BUILD_IMAGES_CONTAINER" ]; then
    # Build in a fresh container.
    docker run --rm \
        -v "$PWD:/code" \
        -e SGX_MODE=SIM \
        -e INTEL_SGX_SDK=/opt/sgxsdk \
        -w /code \
        "$ekiden_image" \
        /code/docker/deployment/build-images-inner.sh
else
    # Build in a specified container.
    docker exec "$BUILD_IMAGES_CONTAINER" \
        /code/docker/deployment/build-images-inner.sh
fi

# Build the deployable image from the output.
docker build --rm --force-rm \
    --build-arg RUNTIME_ETHEREUM_COMMIT_SHA=$runtime_ethereum_commit_sha \
    --build-arg BASE_DOCKER_IMAGE_TAG=$base_docker_image_tag \
    --build-arg RUNTIME_ETHEREUM_BUILD_IMAGE_TAG=$BUILD_IMAGE_TAG \
    -t oasislabs/ekiden-runtime-ethereum:$BUILD_IMAGE_TAG - <target/docker-deployment/context.tar.gz
