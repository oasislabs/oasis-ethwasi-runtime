ARG OASIS_NODE_BASE_DOCKER_IMAGE_TAG=latest

# Load a reference ekiden base image
FROM oasislabs/oasis-node:${OASIS_NODE_BASE_DOCKER_IMAGE_TAG}

ARG OASIS_RUNTIME_COMMIT_SHA
ARG OASIS_RUNTIME_BUILD_IMAGE_TAG

LABEL com.oasislabs.oasis-runtime-commit-sha="${OASIS_RUNTIME_COMMIT_SHA}"
LABEL com.oasislabs.oasis-runtime-build-image-tag="${OASIS_RUNTIME_BUILD_IMAGE_TAG}"

COPY target/release/oasis-runtime /oasis/lib/
COPY target/x86_64-fortanix-unknown-sgx/release/oasis-runtime.sgxs /oasis/lib/
COPY target/release/gateway /oasis/bin/
COPY resources/genesis.json /oasis/res/oasis-runtime-genesis.json
