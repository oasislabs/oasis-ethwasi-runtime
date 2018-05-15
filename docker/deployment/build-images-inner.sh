#!/bin/bash -ex

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
CARGO_TARGET_DIR=target cargo install --force --git https://github.com/oasislabs/ekiden --branch master --debug ekiden-tools
cargo ekiden build-contract --git https://github.com/oasislabs/ekiden --branch master --output target/contract --target-dir target ekiden-key-manager
cargo ekiden build-contract --output-identity
(cd client && CARGO_BUILD_TARGET_DIR=../target cargo build)

# Package all binaries and resources.
mkdir -p target/docker-deployment/context/bin target/docker-deployment/context/lib target/docker-deployment/context/res
ln target/contract/ekiden-key-manager.so target/docker-deployment/context/lib/evm-key-manager.so
ln target/contract/evm.so target/docker-deployment/context/lib
ln target/contract/evm.mrenclave target/docker-deployment/context/res
cp -r resources/genesis target/docker-deployment/context/res
ln target/debug/web3-client target/docker-deployment/context/bin
ln docker/deployment/Dockerfile target/docker-deployment/context/Dockerfile
tar cvzhf target/docker-deployment/context.tar.gz -C target/docker-deployment/context .
rm -rf target/docker-deployment/context
