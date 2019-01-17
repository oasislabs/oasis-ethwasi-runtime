#!/bin/bash -eux

WORKDIR=${1:-$(pwd)}
echo ${WORKDIR}

source scripts/utils.sh

echo "Installing test dependencies"
pushd ${WORKDIR}/tests-e2e/ > /dev/null
npm install
popd > /dev/null

echo "Installing pubsub dependencies."
pushd ${WORKDIR}/tests-e2e/web3js > /dev/null
npm install > /dev/null
popd

echo "Installing wasm32-unknown-unknown target."
rustup target add wasm32-unknown-unknown

echo "Installing wscat."
npm install -g wscat

echo "Installing jq."
apt-get install -y jq

# Only run 'cargo install' if the resulting binaries
# are not already present.
set +u
cargo_install_root=$(get_cargo_install_root)
echo "cargo_install_root=$cargo_install_root"
set -u

if [ ! -e "$cargo_install_root/bin/wasm-build" ]; then
  echo "Installing wasm-build."
  cargo install \
  --git https://github.com/oasislabs/wasm-utils \
  --branch ekiden
fi
