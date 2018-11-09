# TODO: add arg to override this default image
FROM oasislabs/development:0.2.0 as builder

# Add metadata identifying this image as an
# intermediate build image so that we can
# clean it up from our host after a successful
# build. Remember, this intermediate image will
# contain the private SSH key! Best to not
# leave those images lying around where they can
# be forgotten about and lost track of.
LABEL stage=builder

WORKDIR /workdir

# Take an SSH key as a build argument.
ARG SSH_PRIVATE_KEY

ENV SGX_MODE=SIM
ENV INTEL_SGX_SDK=/opt/sgxsdk

# 1. Create the SSH directory.
# 2. Populate the private key file.
# 3. Set the required permissions.
# 4. Add github to our list of known hosts for ssh.
RUN mkdir -p /root/.ssh/ && \
    echo "$SSH_PRIVATE_KEY" > /root/.ssh/id_rsa && \
    chmod -R 600 /root/.ssh/ && \
    ssh-keyscan -t rsa github.com >> ~/.ssh/known_hosts

# 1. Setup ssh-agent env
# 2. Add ssh identity at /root/.ssh/id_rsa
# 3. Tell git to use SSH instead of HTTPS so
#    that we can use the SSH identity when
#    checking out private repos.
# 4. Checkout the runtime-ethereum repo
#
# TODO: Add arg to specify which branch or commit sha should be checked out.
# TODO: Update the git clone command to do a shallow checkout. We only need
#       the latest source to build an image.
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    git clone https://github.com/oasislabs/runtime-ethereum

# Install ekiden-tools
#
# TODO: There is no need to continuously reinstall ekiden-tools
#       all over the place. Instead create an image ekiden/builder
#       or something like that and then use that as the base image
#       for this and other runtime-ethereum CI jobs.
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    CARGO_TARGET_DIR=/workdir/runtime-ethereum/target \
      cargo install \
      --force \
      --git https://github.com/oasislabs/ekiden \
      --branch master \
      ekiden-tools

# Build the runtime
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    cd runtime-ethereum && \
    CARGO_TARGET_DIR=/workdir/runtime-ethereum/target \
      cargo ekiden \
      build-enclave --output-identity \
      --release

# Build the gateway
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    cd runtime-ethereum/gateway && \
    CARGO_TARGET_DIR=/workdir/runtime-ethereum/target \
    cargo build --release

ARG OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG=latest

# TODO: Debug why this errored out with message: "invalid reference format"
# FROM docker.io/oasislabs/testnet:${OASISLABS_TESTNET_BASE_DOCKER_IMAGE_TAG}

# TODO: replace hardcoded "latest" tag with an ARG. See previous TODO.
FROM docker.io/oasislabs/testnet:latest

ARG RUNTIME_ETHEREUM_COMMIT_SHA
ARG RUNTIME_ETHEREUM_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${RUNTIME_ETHEREUM_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${RUNTIME_ETHEREUM_BUILD_IMAGE_TAG}"

COPY --from=builder /workdir/runtime-ethereum/target/enclave/runtime-ethereum.so /ekiden/lib/
COPY --from=builder /workdir/runtime-ethereum/target/enclave/runtime-ethereum.mrenclave /ekiden/res/
COPY --from=builder /workdir/runtime-ethereum/target/release/gateway /ekiden/bin/
