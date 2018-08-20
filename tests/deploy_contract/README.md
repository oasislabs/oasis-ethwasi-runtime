# deploy_contract.js

Deploys a contract stored on disk to a web3 gateway.

## Usage

First `npm install -g .`.
Then `deploy_contract /path/to/contract`.

Parameters:

* `--gateway`: HTTP URL of web3 gateway [default: http://localhost:8545]
* `--dump-json`: print cURLable JSON to stdout
