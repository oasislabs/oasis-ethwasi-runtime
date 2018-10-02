#!/usr/bin/env node

var assert = require('assert');
var Web3 = require('web3');
var Tx = require('ethereumjs-tx');

const web3 = new Web3(new Web3.providers.HttpProvider("localhost:8545"));
web3.eth.defaultAccount = '0x1cca28600d7491365520b31b466f88647b9839ec';

// private key corresponding to defaultAccount. generated from mnemonic:
// patient oppose cotton portion chair gentle jelly dice supply salmon blast priority
const PRIVATE_KEY = new Buffer('c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179', 'hex');

console.log("test2");
