#!/bin/bash -eux

WORKDIR=${1:-$(pwd)}
echo ${WORKDIR}

source scripts/utils.sh

echo "Cloning e2e tests"
if [ ! -d "/e2e-tests" ] ; then
    git clone https://github.com/oasislabs/e2e-tests --branch $E2E_TESTS_BRANCH /e2e-tests
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

echo "Installing unzip."
apt-get install unzip
