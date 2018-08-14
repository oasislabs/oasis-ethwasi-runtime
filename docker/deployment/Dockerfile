ARG BASE_DOCKER_IMAGE_TAG=latest
FROM docker.io/oasislabs/testnet:${BASE_DOCKER_IMAGE_TAG}

ARG RUNTIME_ETHEREUM_COMMIT_SHA
ARG RUNTIME_ETHEREUM_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${RUNTIME_ETHEREUM_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${RUNTIME_ETHEREUM_BUILD_IMAGE_TAG}"

ADD . /ekiden
