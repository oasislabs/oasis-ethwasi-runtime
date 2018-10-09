const fs = require("fs");
const request = require("request-promise");
const HDWalletProvider = require("truffle-hdwallet-provider");
/**
 * _web3 so we don't override truffle's version of web3.
 */
const _web3 = require("web3");
const Tx = require("ethereumjs-tx");

contract("Confidential Counter", async (accounts) => {

  /**
   * Transactions encrypted with the following ephemeral keypair.
   * private key: 0xc61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179
   * address: 0x7110316b618d20d0c44728ac2a3d683536ea682b
   */
  it("deploys, updates, and retrieves the storage of a confidential contract", async () => {
	let counter = new Counter();
	await counter.deploy();
	const firstCounter = await counter.confidential_getCounter(counter);
	await counter.confidential_incrementCounter();
	const secondCounter = await counter.confidential_getCounter(counter);
	// encryption: NONCE = 2 || PK || ENC(0)
	assert.equal(firstCounter.result, "0x000000000000000000000000000000029385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54f6b305ea6e17bfd4db6277ec340e26256a3406c5d38c45f97dd19a66b4a0c25d045d68a4d56f158046b2bd30512798e6");
	// encryption: NONCE = 2 || PK || ENC(1)
	assert.equal(secondCounter.result, "0x000000000000000000000000000000029385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54f6c1360bc673fb58cb0c603eee3b42f11ff4180b604240c3210ad01dbcc3c05c5a3385c1222c119d3069d3ba2edba4f2");
  });

  /**
   * Fails because the state is saved from the previous test and so we expect the next
   * account nonce to be at 2 for the next tx.
   */
  it("should fail when reusing the same nonce for both a non-confidential and confidential tx", async() => {
	let counter = new Counter();
	let nonce = 1;
	const deployTxHash = await counter.deploy(nonce);
	assert.equal(deployTxHash.hasOwnProperty("error"), true);
	const enc_deployTxHash = await counter.confidential_deploy(nonce);
	assert.equal(enc_deployTxHash.hasOwnProperty("error"), true);
  });

  /**
   * Sanity check on successfully sending the above transactions without
   * encryption and with a different account (i.e. a reset nonce).
   */
  it("deploys, updates, and retrieves the storage of a non-confidential contract", async() => {
	let counter = new Counter();
	await counter.deploy(null, null);
	let firstCount = await counter.getCounter();
	await counter.incrementCounter();
	let secondCount = await counter.getCounter();

	assert.equal(firstCount.result, "0x0000000000000000000000000000000000000000000000000000000000000000");
	assert.equal(secondCount.result, "0x0000000000000000000000000000000000000000000000000000000000000001");
  });

  /**
   * Expected encrypted logs.
   */
  // encryption: NONCE = 3 || PK_EPHEMERAL || ENC(1)
  const EXPECTED_LOG_1 = "0x000000000000000000000000000000039385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a548e842fe406af4e0ff75149b60b95a8d3ec09de32f228425c8f237a383122654c46e41ac72b0dd35fc66cf38e3bfe937b";
  // encryption: NONCE = 3 || PK_EPHEMERAL || ENC(2)
  const EXPECTED_LOG_2 = "0x000000000000000000000000000000039385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54fd0e2f257de5f2c99e05a36dc56900ce205c636e542dfbc0a3a6a288dc7fef673843a84e45fc23e5fefaa0eb73764a6e";

  it("encrypts increment count logs in the transaction receipt", async () => {
	// given
	let counter = new Counter();
	await counter.deploy();
	// when
	let txHash = await counter.confidential_incrementCounter();
	let firstReceipt = await makeRpc("eth_getTransactionReceipt", [txHash.result]);
	txHash = await counter.confidential_incrementCounter();
	secondReceipt = await makeRpc("eth_getTransactionReceipt", [txHash.result]);
	// then
	let firstLogCounter = firstReceipt.result.logs[0].data;
	let secondLogCounter = secondReceipt.result.logs[0].data;

	assert.equal(firstLogCounter, EXPECTED_LOG_1);
	assert.equal(secondLogCounter, EXPECTED_LOG_2);
  });

  it("encrypts increment count logs returned by eth_getLogs", async () => {
	// given
	let counter = new Counter();
	await counter.deploy();
	// when
	let txHash = await counter.confidential_incrementCounter();
	let firstReceipt = await makeRpc("eth_getTransactionReceipt", [txHash.result]);
	txHash = await counter.confidential_incrementCounter();
	secondReceipt = await makeRpc("eth_getTransactionReceipt", [txHash.result]);
	// then
	let logs = (await makeRpc("eth_getLogs", [{
	  "fromBlock": "earliest",
	  "toBlock": "latest",
	  "address": counter.contractAddress,
	}])).result;
	assert.equal(logs[0].data, EXPECTED_LOG_1);
	assert.equal(logs[1].data, EXPECTED_LOG_2);
  });
})

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
	uri: "http://127.0.0.1:8545",
	body: JSON.stringify(body)
  };
  return JSON.parse(await request(options));
}

/**
 * To generate the encrypted transactions, take the manual transactions generated
 * here and encrypt the data field before serializing the transactions.
 */
class Counter {

  constructor() {
	this.contractAddress = "";
	this.artifact = JSON.parse(fs.readFileSync("./build/contracts/Counter.json").toString());
	this.contract = new (new _web3()).eth.Contract(this.artifact.abi);
	this._setupKeys();
  }

  _setupKeys() {
	this.signerAddress = "0x1cca28600d7491365520b31b466f88647b9839ec";
	let mnemonic = 'patient oppose cotton portion chair gentle jelly dice supply salmon blast priority';
	let provider = new HDWalletProvider(mnemonic, "http://localhost:8545");
	this.privateKey = provider.wallets[this.signerAddress]._privKey;
  }

  async deploy(nonce) {
	let data = this.contract.deploy({data: this.artifact.bytecode}).encodeABI();
	return this._deploy(nonce, data, "eth_sendRawTransaction");
  }

  async confidential_deploy(nonce) {
	const encryptedDeployData = "0x000000000000000000000000000000015ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d5215713b1a9e39c17118166523bd5b32ad3e323cf7a6f6fd824fcae3cd1b4ce07d41b668e4e87563c7ca46936999b1ac3f666dd04f2babd234a3530747d0cfaccbaa1644554f1590aedea54c9dc7506813d76510dd372220c0048fccbcb67a113f242188c45a1a08882613fab0be12de016a13920ef362c8e8f041707a1493dad3a8bcd4ba313daca528c456a9bb3521459a26f3a4e78f1bfe1bee04bb7da5e1c80ff7e047ff0185d95833efec817122db2a4274ab994bacb10fdb3d48b579fd7af1dd26cdfc10824281d241890daf3287b97cdee8edfdfa8ea5e2cd3376a36622e696c8ef7d766ab93c3fc6b1b3e1dd509dabe8567e6d4ec79c96ed89cfe9e9f50f3f2d80f957fc89898d5f207312df4b262adef852de5c81e739e164b42ca7a9fdd39fe239888622df1f6308f89d2f4de5ce973d14dad5c9cf8fb";
	return this._deploy(nonce, encryptedDeployData, "confidential_sendRawTransaction");
  }

  /**
   * @param nonce is optional and is used to override the nonce parameter in the
   *        transaction, for example, if we purposefully want to send a transaction
   *        with a stale nonce.
   */
  async _deploy(nonce, deployData, endpoint) {
	if (!nonce) {
	  nonce = (await fetchNonce(this.signerAddress)).result
	}
	const deployTx = new Tx({
	  data: deployData,
	  gasLimit: '0x100000',
	  from: this.signerAddress,
	  value: 0,
	  gasPrice: '0x3b9aca00',
	  nonce: nonce
	});
	deployTx.sign(this.privateKey);
	const rawTx = '0x' + deployTx.serialize().toString('hex');
	let txHash = await makeRpc(endpoint, [rawTx]);
	let receipt = await makeRpc("eth_getTransactionReceipt", [txHash.result]);
	if (receipt.result) {
	  let contractAddr = receipt.result.contractAddress;
	  this.contractAddress = contractAddr;
	  return contractAddr;
	}
	// error
	return receipt;
  }

  async getCounter() {
	const data = this.contract.methods.getCounter().encodeABI();
	return this._getCounter("eth_call", data);
  }

  async confidential_getCounter() {
	const encryptedData = "0x000000000000000000000000000000025ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d5218664c085073d1b0ba8b2b11b52b1a094da71c01f";
	return this._getCounter("confidential_call_enc", encryptedData);
  }

  async _getCounter(endpoint, data) {
	const params = [
	  {
		"to": this.contractAddress,
		"data": data
	  },
	  "latest"
	];
	return makeRpc(endpoint, params);
  }

  async confidential_incrementCounter() {
	const encryptedIncrementData = "0x000000000000000000000000000000035ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d52153c65572cc810f0cc2f3a1568c8c9330e8b12c90";
	const endpoint = "confidential_sendRawTransaction";
	return this._incrementCounter(endpoint, encryptedIncrementData);
  }


  async incrementCounter() {
	const incrementCounterData = this.contract.methods.incrementCounter().encodeABI();
	const endpoint = "eth_sendRawTransaction";
	return this._incrementCounter(endpoint, incrementCounterData);
  }

  async _incrementCounter(endpoint, incrementCounterData) {
	const incrementTx = new Tx({
	  data: incrementCounterData,
	  gasLimit: '0x100000',
	  to: this.contractAddress,
	  from: this.signerAddress,
	  value: 0,
	  gasPrice: '0x3b9aca00',
	  nonce: (await fetchNonce(this.signerAddress)).result
	});
	incrementTx.sign(this.privateKey);
	let incrementRawTx = '0x' + incrementTx.serialize().toString('hex');
	return makeRpc(endpoint, [incrementRawTx]);
  }
}
