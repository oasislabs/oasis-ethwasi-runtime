ARG OASISLABS_EKIDEN_RUNTIME_ETHEREUM_BASE_DOCKER_IMAGE_TAG=latest-testing

# Compiled runtime-ethereum image.
FROM oasislabs/ekiden-runtime-ethereum:${OASISLABS_EKIDEN_RUNTIME_ETHEREUM_BASE_DOCKER_IMAGE_TAG} as ekiden-runtime-ethereum

# Image containing go and rust tools.
FROM oasislabs/development:0.3.0 as builder

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
# Take commit SHA as build argument.
ARG RUNTIME_ETHEREUM_COMMIT_SHA

# 1. Create the SSH directory.
# 2. Populate the private key file.
# 3. Set the required permissions.
# 4. Add github to our list of known hosts for ssh.
RUN mkdir -p /root/.ssh/ && \
    echo "$SSH_PRIVATE_KEY" > /root/.ssh/id_rsa && \
    chmod -R 600 /root/.ssh/ && \
    ssh-keyscan -t rsa github.com >> ~/.ssh/known_hosts

# 1. Set protobuf branch to master, otherwise go-ethereum make fails, because
#    git pull on protobuf fails.
# 2. Compile&Install go-ethereum devtools (abigen etc.)
# 3. Install cargo wasm32-unknown-unknown target
# 4. Install wasm-build command
# 5. Install xxd command
RUN cd /go/src/github.com/golang/protobuf && git checkout master && \
    go get -d github.com/ethereum/go-ethereum && \
    cd /go/src/github.com/ethereum/go-ethereum/ && \
    make devtools && \
    rustup target add wasm32-unknown-unknown && \
    cargo install owasm-utils-cli --bin wasm-build && \
    apt install -y xxd

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
    git clone https://github.com/oasislabs/runtime-ethereum && \
    cd runtime-ethereum && \
    git checkout ${RUNTIME_ETHEREUM_COMMIT_SHA}

# 1. Compile genesis tool.
# 2. Compile benchmarking client containing benchmarks and benchmarking smart
#    contracts written in rust.
RUN eval $(ssh-agent -s) && \
    ssh-add && \
    git config --global url."ssh://git@github.com".insteadOf "https://github.com" && \
    cd runtime-ethereum && \
    CARGO_TARGET_DIR=/workdir/runtime-ethereum/target && \
    cargo build -p genesis --release && \
    cd benchmark && \
    make

FROM ekiden-runtime-ethereum

ARG RUNTIME_ETHEREUM_COMMIT_SHA
ARG RUNTIME_ETHEREUM_BUILD_IMAGE_TAG

LABEL com.oasislabs.runtime-ethereum-commit-sha="${RUNTIME_ETHEREUM_COMMIT_SHA}"
LABEL com.oasislabs.runtime-ethereum-build-image-tag="${RUNTIME_ETHEREUM_BUILD_IMAGE_TAG}"

COPY --from=builder /workdir/runtime-ethereum/target/release/genesis /ekiden/bin/
COPY --from=builder /workdir/runtime-ethereum/benchmark/benchmark /ekiden/bin/
