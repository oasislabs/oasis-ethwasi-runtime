FROM docker.io/tozd/sgx:ubuntu-bionic

RUN apt-get install -qq libsnappy1v5 librocksdb5.8 && \
    wget http://archive.ubuntu.com/ubuntu/pool/main/b/bubblewrap/bubblewrap_0.4.1-1_amd64.deb && \
    echo '25de452f209e4fdb4b009851c33ca9a0269ebf0b92f4bd9b86186480592cc3e2 bubblewrap_0.4.1-1_amd64.deb' | sha256sum -c && \
    dpkg -i bubblewrap_0.4.1-1_amd64.deb && \
    rm bubblewrap_0.4.1-1_amd64.deb

ARG OASIS_CORE_VERSION
ARG OASIS_RUNTIME_COMMIT_SHA
ARG OASIS_RUNTIME_BUILD_IMAGE_TAG

LABEL com.oasislabs.oasis-core-version="${OASIS_CORE_VERSION}"
LABEL com.oasislabs.oasis-runtime-commit-sha="${OASIS_RUNTIME_COMMIT_SHA}"
LABEL com.oasislabs.oasis-runtime-build-image-tag="${OASIS_RUNTIME_BUILD_IMAGE_TAG}"

# Oasis Core artifacts.
COPY oasis-core/oasis-node /oasis/bin/oasis-node
COPY oasis-core/oasis-core-runtime-loader /oasis/bin/

# Oasis runtime.
COPY target/release/oasis-ethwasi-runtime /oasis/lib/oasis-runtime
COPY target/x86_64-fortanix-unknown-sgx/release/oasis-ethwasi-runtime.sgxs /oasis/lib/oasis-runtime.sgxs
# Gateway.
COPY target/release/gateway /oasis/bin/
COPY resources/genesis.json /oasis/res/oasis-runtime-genesis.json
# Keymanager runtime.
COPY target/release/oasis-ethwasi-runtime-keymanager /oasis/lib/oasis-runtime-keymanager
COPY target/x86_64-fortanix-unknown-sgx/release/oasis-ethwasi-runtime-keymanager.sgxs /oasis/lib/oasis-runtime-keymanager.sgxs

ENV PATH "/oasis/bin:${PATH}"
