const fs = require('fs');
const request = require("request-promise");
const HDWalletProvider = require("truffle-hdwallet-provider");

const GAS_PRICE = '0x3b9aca00';
const GAS_LIMIT = '0x100000';
const _CONFIDENTIAL_PREFIX = '00707269';
const _GATEWAY_URL = "http://localhost:8545";

/**
 * Returns a contract build artifact containing the abi and bytecode.
 * Assumes all files are compiled with truffle compile before hand.
 */
function readArtifact(contractName) {
  const path = './build/contracts/' + contractName + '.json';
  return JSON.parse(fs.readFileSync(path).toString());
}

function provider() {
  let mnemonic = 'patient oppose cotton portion chair gentle jelly dice supply salmon blast priority';
  let provider = new HDWalletProvider(mnemonic, _GATEWAY_URL);
  let address = Object.keys(provider.wallets)[0];
  return {
	provider,
	address,
	privateKey: provider.wallets[address]._privKey
  }
}

/**
 * Returns a confidential version of the initcode such that, if it's used in a
 * transaction, it will create a confidential contract.
 */
function makeConfidential(initcodeHex) {
  return "0x" + _CONFIDENTIAL_PREFIX + initcodeHex.substr(2);
}

async function fetchNonce(address) {
  return makeRpc("eth_getTransactionCount", [address, "latest"]);
}

async function makeRpc(method, params) {
  let body = {
	"method": method,
	"id": 1,
	"jsonrpc": "2.0",
	"params": params
  };
  let options = {
	headers: {
	  "Content-type": "application/json"
	},
	method: "POST",
	uri: _GATEWAY_URL,
	body: JSON.stringify(body)
  };
  return JSON.parse(await request(options));
}

module.exports = {
  readArtifact,
  provider,
  fetchNonce,
  makeRpc,
  makeConfidential,
  GAS_LIMIT,
  GAS_PRICE,
}
