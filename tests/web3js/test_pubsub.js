#!/usr/bin/env node

let fs = require('fs');
let program = require('commander');
let Web3 = require('web3');
let Tx = require('ethereumjs-tx');

const web3 = new Web3(new Web3.providers.WebsocketProvider('http://localhost:8555'));
web3.eth.defaultAccount = '0x1cca28600d7491365520b31b466f88647b9839ec';

var subscription = web3.eth.subscribe('logs', { "fromBlock":"latest", "toBlock":"latest"},
  function(error, result){
    if (!error)
        console.log(result);
  }
)
.on("data", function(log){
    console.log(log);
})
.on("changed", function(log){
    console.log(log);
});
console.log(subscription);

setTimeout(function() {
  subscription.unsubscribe(function(err, _) {
    if (!err)
      console.log("Unsubscribed");
    process.exit();
  });
}, 240000)
