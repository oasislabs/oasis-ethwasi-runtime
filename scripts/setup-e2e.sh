#!/bin/bash -eux

WORKDIR=${1:-$(pwd)}
echo ${WORKDIR}

source scripts/utils.sh

echo "Cloning e2e tests"
if [ ! -d "/e2e-tests" ] ; then
    git clone https://github.com/oasislabs/e2e-tests --branch beta /e2e-tests
fi
echo "Installing e2e test dependencies"
pushd /e2e-tests/ > /dev/null
npm install
popd > /dev/null

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
