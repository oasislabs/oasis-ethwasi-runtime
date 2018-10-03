echo "Installing truffle-hdwallet-provider."
# Temporary fix for ethereumjs-wallet@0.6.1 incompatibility
npm install ethereumjs-wallet@=0.6.0
npm install truffle-hdwallet-provider

echo "Installing wasm32-unknown-unknown target."
rustup target add wasm32-unknown-unknown

echo "Installing wscat."
npm install -g wscat

echo "Installing jq."
apt-get install -y jq

