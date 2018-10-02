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
   * private key: c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179
   * address: 0x7110316b618d20d0c44728ac2a3d683536ea682b
   */
  it("deploys, updates, and retrieves the storage of a confidential contract", async () => {
	let counter = new Counter();
	await counter.confidential_deploy();
	const firstCounter = await counter.confidential_getCounter(counter);
	await counter.confidential_incrementCounter();
	const secondCounter = await counter.confidential_getCounter(counter);
	// encryption: NONCE = 2 || PK || ENC(0)
	assert.equal(firstCounter.result, "0x000000000000000000000000000000029385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54f6b305ea6e17bfd4db6277ec340e26256a3406c5d38c45f97dd19a66b4a0c25d045d68a4d56f158046b2bd30512798e6");
	// encryption: NONCE = 2 || PK || ENC(1)
	assert.equal(secondCounter.result, "0x000000000000000000000000000000029385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54f6c1360bc673fb58cb0c603eee3b42f11ff4180b604240c3210ad01dbcc3c05c5a3385c1222c119d3069d3ba2edba4f2");

  })

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
  })
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
  })
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
	const encryptedDeployData = "0x000000000000000000000000000000015ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d521a533fd24fdf40679044ca2fd758a58011c0b93784fea3741c8a9c772e67f5b83b484f50ff87744dfad83d5e7266c983325c718ef822901c7a43d734af0c6924813dd1acc64a4b32c93afb1efdc6f710dd5f5635d37befc3b2e112a518b1b115d7e5dafa492c936bd0fc64dbd307b76c25e1531b23a78bc1c05ba6b5a22f0c35b6c83f3eb5aaee62016362be90059a9d0dcc788d13b8eaecdaadd30bac4e5cc71232190de9145d87228fef77367e1bc0c676829d7895af66bf3e030715247603f8f912f877c8c44b3051695bde7559ab036db3f73fe4cabd08c0b988291a45a83300af8f8f433eea9443cd5eb034bea8e9efae766e17133059acfb75544ca060e";
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
