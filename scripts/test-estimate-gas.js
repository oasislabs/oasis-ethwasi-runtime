const Web3c = require('web3c');
const utils = require('./utils');

const Counter = artifacts.require('Counter');
const ConfidentialCounter = artifacts.require('ConfidentialCounter');
const WasmCounter = artifacts.require('WasmCounter');
const ConfidentialWasmCounter = artifacts.require('ConfidentialWasmCounter');

contract('Esttimate Gas', async (accounts) => {
  it('should estimate gas for wasm transactions the same as gas actually used', async () => {
    let counterContract = new web3c.confidential.Contract(Counter.abi);
    const deployMethod = counterContract.deploy({ data: Counter.bytecode });
    let estimatedGas = await deployMethod.estimateGas();
    counterContract = await deployMethod.send({
      from: accounts[0],
      gasPrice: '0x3b9aca00',
      gas: estimatedGas
    });
    const txHash = counterContract._requestManager.provider.outstanding[0];
    const receipt = await web3c.eth.getTransactionReceipt(txHash);

    assert.equal(estimatedGas, receipt.gasUsed);
    assert.equal(estimatedGas, receipt.cumulativeGasUsed);
  });
});
