ARG OASIS_RUNTIME_BASE_DOCKER_IMAGE_TAG=master-benchmarking

FROM oasislabs/oasis-runtime:${OASIS_RUNTIME_BASE_DOCKER_IMAGE_TAG}

ARG OASIS_RUNTIME_COMMIT_SHA
ARG OASIS_RUNTIME_BUILD_IMAGE_TAG

LABEL com.oasislabs.oasis-runtime-commit-sha="${OASIS_RUNTIME_COMMIT_SHA}"
LABEL com.oasislabs.oasis-runtime-build-image-tag="${OASIS_RUNTIME_BUILD_IMAGE_TAG}"

COPY target/release/genesis-playback /oasis/bin/
COPY benchmark/benchmark /oasis/bin/
