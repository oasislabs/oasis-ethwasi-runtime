#!/bin/bash -e

# Our development image sets up the PATH in .bashrc. Source that.
PS1='\$'
. ~/.bashrc

# Abort on unclean packaging area.
if [ -e target/docker-deployment/context ]; then
    cat >&2 <<EOF
Path target/docker-deployment/context already exists. Aborting.
If this was accidentally left over and you don't need anything from
it, you can remove it and try again.
EOF
    exit 1
fi

# Build all Ekiden binaries and resources.
# TODO: Check if we can save any compilation by setting CARGO_TARGET_DIR.
cargo install --force --git https://github.com/oasislabs/ekiden --tag 0.1.0-alpha.3 ekiden-tools
# TODO: Let this share our target path to reduce duplicated compilation.
cargo ekiden build-contract --git https://github.com/oasislabs/ekiden --tag 0.1.0-alpha.3 --output target/contract --release ekiden-key-manager
cargo ekiden build-contract --release
(cd client && cargo build --release)

# Package all binaries and resources.
mkdir -p target/docker-deployment/context/bin target/docker-deployment/context/lib
ln target/contract/ekiden-key-manager.so target/docker-deployment/context/lib/evm-key-manager.so
ln target/contract/evm.so target/docker-deployment/context/lib
ln client/target/release/web3-client target/docker-deployment/context/bin
ln docker/deployment/Dockerfile target/docker-deployment/context/Dockerfile
tar cvzhf target/docker-deployment/context.tar.gz -C target/docker-deployment/context .
rm -rf target/docker-deployment/context
