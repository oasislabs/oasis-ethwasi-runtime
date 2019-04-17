ARG OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG=latest

# Load a reference testnet base image
FROM oasislabs/testnet:${OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG}

ARG RUNTIME_ETHEREUM_COMMIT_SHA
ARG RUNTIME_ETHEREUM_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${RUNTIME_ETHEREUM_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${RUNTIME_ETHEREUM_BUILD_IMAGE_TAG}"

COPY target/release/runtime-ethereum /ekiden/lib/
COPY target/x86_64-fortanix-unknown-sgx/release/runtime-ethereum.sgxs /ekiden/lib/
COPY target/release/gateway /ekiden/bin/
COPY resources/genesis.json /ekiden/res/runtime-ethereum-genesis.json
