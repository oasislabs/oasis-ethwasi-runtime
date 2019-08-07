ARG OASISLABS_RUNTIME_BASE_DOCKER_IMAGE_TAG=latest-testing

FROM oasislabs/ekiden-runtime-ethereum:${OASISLABS_RUNTIME_BASE_DOCKER_IMAGE_TAG}

ARG RUNTIME_ETHEREUM_COMMIT_SHA
ARG RUNTIME_ETHEREUM_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${RUNTIME_ETHEREUM_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${RUNTIME_ETHEREUM_BUILD_IMAGE_TAG}"

COPY target/release/genesis-playback /ekiden/bin/
COPY benchmark/benchmark /ekiden/bin/
