FROM docker.io/tozd/sgx:ubuntu-bionic

RUN apt-get install -qq libsnappy1v5 librocksdb5.8 && \
    wget http://archive.ubuntu.com/ubuntu/pool/main/b/bubblewrap/bubblewrap_0.3.3-2_amd64.deb && \
    echo '1cf9bdae5cfab75f292fad9ee0ef76a7c55243dbc0515709624b2a9573d19447 bubblewrap_0.3.3-2_amd64.deb' | sha256sum -c && \
    dpkg -i bubblewrap_0.3.3-2_amd64.deb && \
    rm bubblewrap_0.3.3-2_amd64.deb

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
COPY target/release/oasis-runtime /oasis/lib/
COPY target/x86_64-fortanix-unknown-sgx/release/oasis-runtime.sgxs /oasis/lib/
# Gateway.
COPY target/release/gateway /oasis/bin/
COPY resources/genesis.json /oasis/res/oasis-runtime-genesis.json
# Keymanager runtime.
COPY target/release/oasis-runtime-keymanager /oasis/lib/
COPY target/x86_64-fortanix-unknown-sgx/release/oasis-runtime-keymanager.sgxs /oasis/lib/

ENV PATH "/oasis/bin:${PATH}"
