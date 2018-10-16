/**
 * _web3 so we don't override truffle's version of web3.
 */
const _web3 = require("web3");
const Tx = require("ethereumjs-tx");
const utils = require("./utils");

contract("Confidential Contracts", async (accounts) => {

  const provider = utils.provider();

  it("stores the long term public key in the deploy logs", async () => {
	const artifact = utils.readArtifact("Counter");
	const counterContract = new (new _web3()).eth.Contract(artifact.abi);
	const initcode = counterContract.deploy({data: artifact.bytecode}).encodeABI();
	const deployData = utils.makeConfidential(initcode);
	const deployTx = new Tx({
	  data: deployData,
	  gasLimit: utils.GAS_LIMIT,
	  from: provider.address,
	  value: 0,
	  gasPrice: utils.GAS_PRICE,
	  nonce: (await utils.fetchNonce(provider.address)).result
	});
	deployTx.sign(provider.privateKey);
	const rawTx = '0x' + deployTx.serialize().toString('hex');

	let txHash = await utils.makeRpc("eth_sendRawTransaction", [rawTx]);
	let receipt = await utils.makeRpc("eth_getTransactionReceipt", [txHash.result]);
	let log = receipt.result.logs[0];

	assert.equal(log.topics[0], "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
	assert.equal(log.data, "0x9385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54");
	assert.equal(log.logIndex, "0x0");
  });
});
