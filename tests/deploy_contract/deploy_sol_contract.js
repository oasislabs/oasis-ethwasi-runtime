#!/usr/bin/env node

let fs = require('fs');
let program = require('commander');
let Web3 = require('web3');
let solc = require('solc');
let Tx = require('ethereumjs-tx');

const web3 = new Web3(new Web3.providers.HttpProvider(program.gateway));
web3.eth.defaultAccount = '0x1cca28600d7491365520b31b466f88647b9839ec';

// private key corresponding to defaultAccount. generated from mnemonic:
// patient oppose cotton portion chair gentle jelly dice supply salmon blast priority
const PRIVATE_KEY = new Buffer('c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179', 'hex');

program
  .option('--gateway <gateway>', 'gateway http address', 'http://localhost:8545')
  .option('--gas-limit <limit>', 'contract gas limit', '0x1e84800')
  .option('--gas-price <price>', 'contract gas price', '0x3b9aca00')
  .option('--dump-json', 'dump cURLable json')
  .parse(process.argv);

console.log('reading contract');
const contractFilename = program.args[0];
const input = fs.readFileSync(contractFilename).toString();
console.log(input);

console.log('compiling contract');
var contract = solc.compile(input, 1);

let Event = new web3.eth.Contract(JSON.parse(contract.contracts[':Event'].interface));

web3.eth.getTransactionCount(web3.eth.defaultAccount).then(nonce => {
  const tx = new Tx({
    data: '0x' + contract.contracts[':Event'],
    gasLimit: program.gasLimit,
    gasPrice: program.gasPrice,
    nonce: nonce,
    value: 0,
  });
  tx.sign(PRIVATE_KEY);
  console.log('signed transaction');

  let serializedTx = '0x' + tx.serialize().toString('hex');

  if (program.dumpJson) {
    console.log(JSON.stringify({
      jsonrpc: '2.0',
      id: 2,
      method: 'eth_sendRawTransaction',
      params: [serializedTx],
    }));
    return;
  }

  return web3.eth.sendSignedTransaction(serializedTx).then(receipt => {
    console.log('submitted tx');
    console.log(receipt);
    console.log(receipt.contractAddress);
    Event.options.address = receipt.contractAddress;
    let transaction = Event.methods.emitEvent(123);
    console.log(transaction.log);
    process.exit();
  });
}).catch(err => {
  console.error('ERROR: Could not deploy contract')
  console.error(err)
});
