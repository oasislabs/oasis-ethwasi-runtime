#!/usr/bin/env node

let fs = require('fs');
let program = require('commander');
let Web3 = require('web3');
let Tx = require('ethereumjs-tx');

program
  .option('--contract-address <address>', 'Contract address', '0x00000000000000000000000000000000000000')
  .parse(process.argv);

const web3 = new Web3(new Web3.providers.HttpProvider('http://localhost:8545'));
web3.eth.defaultAccount = '0x1cca28600d7491365520b31b466f88647b9839ec';

web3.eth.call({to: program.contractAddress,}).then(result => {
    console.log("called contract");
    console.log(result);
    process.exit();
})
.catch(function(error) {
    console.log(error);
    process.exit();
});

