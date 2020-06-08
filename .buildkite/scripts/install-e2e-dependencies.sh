#! /bin/bash

set -euxo pipefail

curl -o- https://raw.githubusercontent.com/creationix/nvm/v0.33.11/install.sh | bash
export NVM_DIR="${HOME}/.nvm"

. $NVM_DIR/nvm.sh
nvm install lts/carbon --latest-npm
nvm use lts/carbon
nvm alias default node
npm install -g truffle-oasis
