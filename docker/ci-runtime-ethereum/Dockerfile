# TODO: add arg to override this default image
FROM oasislabs/testing:0.2.0 as builder

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

ENV SGX_MODE="SIM"
ENV INTEL_SGX_SDK="/opt/sgxsdk"

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
# 3. Checkout the runtime-ethereum repo
#
# TODO: Add arg to specify which branch or commit sha should be checked out.
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git clone \
      --depth 1 \
      git@github.com:oasislabs/ekiden.git

# 1. Setup ssh-agent env
# 2. Add ssh identity at /root/.ssh/id_rsa
# 3. Tell git to use SSH instead of HTTPS so
#    that we can use the SSH identity when
#    checking out private repos during 'cargo install'.
# 4. Install ekiden-tools
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    CARGO_INSTALL_ROOT="/root/.cargo" \
      cargo install \
      --path /workdir/ekiden/tools \
      ekiden-tools

# 1. Setup ssh-agent env
# 2. Add ssh identity at /root/.ssh/id_rsa
# 3. Tell git to use SSH instead of HTTPS so
#    that we can use the SSH identity when
#    checking out private repos during 'cargo install'.
# 4. Install ekiden-compute
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    CARGO_INSTALL_ROOT="/root/.cargo" \
      cargo install \
      --path /workdir/ekiden/compute \
      --debug \
      ekiden-compute

# 1. Setup ssh-agent env
# 2. Add ssh identity at /root/.ssh/id_rsa
# 3. Tell git to use SSH instead of HTTPS so
#    that we can use the SSH identity when
#    checking out private repos during 'cargo install'.
# 4. Install cargo-tarpaulin
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    CARGO_INSTALL_ROOT="/root/.cargo" \
    RUSTFLAGS="--cfg procmacro2_semver_exempt" \
      cargo install \
      --git https://github.com/oasislabs/tarpaulin \
      --branch ekiden \
      cargo-tarpaulin

# 1. Setup ssh-agent env
# 2. Add ssh identity at /root/.ssh/id_rsa
# 3. Tell git to use SSH instead of HTTPS so
#    that we can use the SSH identity when
#    checking out private repos during 'cargo install'.
# 4. Install wasm-utils
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    CARGO_INSTALL_ROOT="/root/.cargo" \
      cargo install \
      --git https://github.com/oasislabs/wasm-utils \
      --branch ekiden

# TODO: add arg to override this default image
FROM oasislabs/testing:0.2.0

ARG EKIDEN_COMMIT_SHA
ARG BUILD_IMAGE_TAG=latest

LABEL com.oasislabs.ekiden-commit-sha="${EKIDEN_COMMIT_SHA}"
LABEL com.oasislabs.ci-runtime-ethereum-build-image-tag="${BUILD_IMAGE_TAG}"

# Copy installed cargo-ekiden (ekiden-tools) binary
COPY --from=builder /root/.cargo/bin/cargo-ekiden /root/.cargo/bin/cargo-ekiden

# Copy installed ekiden-compute binary
COPY --from=builder /root/.cargo/bin/ekiden-compute /root/.cargo/bin/ekiden-compute

# Copy installed cargo-tarpaulin binary
COPY --from=builder /root/.cargo/bin/cargo-tarpaulin /root/.cargo/bin/cargo-tarpaulin

# Copy installed wasm-utils binary
COPY --from=builder /root/.cargo/bin/wasm-utils /root/.cargo/bin/wasm-utils
