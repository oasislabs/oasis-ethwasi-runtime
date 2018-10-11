WORKDIR=${1:-$(pwd)}
echo ${WORKDIR}

echo "Installing test dependencies"
pushd ${WORKDIR}/tests/ > /dev/null
npm install
popd > /dev/null

echo "Installing pubsub dependencies."
pushd ${WORKDIR}/tests/web3js > /dev/null
npm install > /dev/null
popd

echo "Installing wasm32-unknown-unknown target."
rustup target add wasm32-unknown-unknown

echo "Installing wscat."
npm install -g wscat

echo "Installing jq."
apt-get install -y jq

echo "Installing wasm-build."
cargo install --git https://github.com/oasislabs/wasm-utils --branch ekiden
