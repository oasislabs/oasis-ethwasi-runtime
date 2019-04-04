ARG OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG=latest

# Load a reference testnet base image
FROM oasislabs/testnet:${OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG}

ARG RUNTIME_ETHEREUM_COMMIT_SHA
ARG RUNTIME_ETHEREUM_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${RUNTIME_ETHEREUM_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${RUNTIME_ETHEREUM_BUILD_IMAGE_TAG}"

COPY target/enclave/runtime-ethereum.so /ekiden/lib/
COPY target/enclave/runtime-ethereum.mrenclave /ekiden/res/
COPY target/release/gateway /ekiden/bin/
