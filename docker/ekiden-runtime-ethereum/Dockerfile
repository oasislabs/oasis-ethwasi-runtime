ARG OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG=latest-devnet2

# Load a reference testnet base image
FROM oasislabs/testnet:${OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG}

# The naming here is a temporary measure to support devnet2. This should not be
# merged upstream once devnet2 is destroyed.
ARG OASIS_RUNTIME_COMMIT_SHA
ARG OASIS_RUNTIME_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${OASIS_RUNTIME_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${OASIS_RUNTIME_BUILD_IMAGE_TAG}"
LABEL com.oasislabs.oasis-runtime-commit-sha="${OASIS_RUNTIME_COMMIT_SHA}"
LABEL com.oasislabs.oasis-runtime-build-image-tag="${OASIS_RUNTIME_BUILD_IMAGE_TAG}"

COPY target/release/runtime-ethereum /ekiden/lib/
COPY target/x86_64-fortanix-unknown-sgx/release/runtime-ethereum.sgxs /ekiden/lib/
COPY target/release/gateway /ekiden/bin/
COPY resources/genesis.json /ekiden/res/runtime-ethereum-genesis.json
