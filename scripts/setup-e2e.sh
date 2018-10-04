echo "Installing test dependencies"
pushd ${WORKDIR}/tests/ > /dev/null
npm install
popd > /dev/null

echo "Installing wasm32-unknown-unknown target."
rustup target add wasm32-unknown-unknown

echo "Installing wscat."
npm install -g wscat

echo "Installing jq."
apt-get install -y jq

