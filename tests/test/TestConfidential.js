const fs = require("fs");
const request = require("request-promise");
const web3 = require("web3");
const Tx = require("ethereumjs-tx");

contract("Confidential Counter", async (accounts) => {

  /**
   * Transactions signed with
   * private key: c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179
   * address: 0x7110316b618d20d0c44728ac2a3d683536ea682b
   */
  it("deploys, updates, and retrieves the storage of a confidential contract", async () => {
	const deployTxHash = await deployCounterContractEnc();
	const firstCounter = await getCounter1Enc();
	const incrementTxHash = await incrementCounterEnc();
	const secondCounter = await getCounter2Enc();

	assert.equal(deployTxHash.result, "0x02a4617df259c63185850d5b4104a94967dc06f41b44caf531de7b09a6c8a803");
	// encryption: NONCE = 2 || PK || ENC(0)
	assert.equal(firstCounter.result, "0x000000000000000000000000000000029385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54f6b305ea6e17bfd4db6277ec340e26256a3406c5d38c45f97dd19a66b4a0c25d045d68a4d56f158046b2bd30512798e6");
	assert.equal(incrementTxHash.result, "0x8d34c357f314e169da5ba90e4b4746304411c280d207b31d77aa7e719f5d116c");
	// encryption: NONCE = 4 || PK || ENC(1)
	assert.equal(secondCounter.result, "0x000000000000000000000000000000049385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54df4e6705a85c78692165238b7094bc1f280222c30347147e0d88ec5c4d9435d0483c4aa57de82571ec5b8f8489e02e56");
  })

  it("should fail when reusing the same nonce for both a non-confidential and confidential tx", async() => {
	let counter = new Counter(
	  "0x7110316b618d20d0c44728ac2a3d683536ea682b",
	  new Buffer("c61675c22aee77da8f6e19444ece45557dc80e1482aa848f541e94e3e5d91179", "hex")
	);
	const deployTxHash = await counter.deploy();
	assert.equal(deployTxHash.hasOwnProperty("error"), true);
  })

  /**
   * Sanity check on successfully sending the above transactions without
   * encryption and with a different account (i.e. a reset nonce).
   */
  it("deploys, updates, and retrieves the storage of a non-confidential contract", async() => {
	let account = (new web3()).eth.accounts.create();
	const privateKey = new Buffer(account.privateKey.substr(2), "hex");

	let counter = new Counter(account.address, privateKey);
	await counter.deploy();
	let firstCount = await counter.getCounter();
	await counter.incrementCounter();
	let secondCount = await counter.getCounter();
	assert.equal(firstCount.result, "0x0000000000000000000000000000000000000000000000000000000000000000");
	assert.equal(secondCount.result, "0x0000000000000000000000000000000000000000000000000000000000000001");
  })
})

async function deployCounterContractEnc() {
  const raw_tx = "0xf9017e8080831000008080b90130000000000000000000000000000000015ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d521a533fd24fdf40679044ca2fd758a58011c0b93784fea3741c8a9c772e67f5b83b484f50ff87744dfad83d5e7266c983325c718ef822901c7a43d734af0c6924813dd1acc64a4b32c93afb1efdc6f710dd5f5635d37befc3b2e112a518b1b115d7e5dafa492c936bd0fc64dbd307b76c25e1531b23a78bc1c05ba6b5a22f0c35b6c83f3eb5aaee62016362be90059a9d0dcc788d13b8eaecdaadd30bac4e5cc71232190de9145d87228fef77367e1bc0c676829d7895af66bf3e030715247603f8f912f877c8c44b3051695bde7559ab036db3f73fe4cabd08c0b988291a45a83300af8f8f433eea9443cd5eb034bea8e9efae766e17133059acfb75544ca060e1ca030be29abaff42bc41ece3280c8336a044c291b8b9b8709f09ba6a4144f61c72fa041b02916948bd818d1c683ea44e07ddd0928bc0f96098d2688bc1db93c46806a";

  return makeRpc("confidential_sendRawTransaction", [raw_tx]);
}

/**
 * Separate out the getCounter1 and 2 functions since they are
 * encrypted with different nonces.
 */
async function getCounter1Enc() {
  return _getCounterEnc(
	// encrypted calldata: IV = 2 || PK || ENC(calldata)
	"0x000000000000000000000000000000025ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d5218664c085073d1b0ba8b2b11b52b1a094da71c01f"
  );
}

async function getCounter2Enc() {
  return _getCounterEnc(
	// encrypted calldata: IV = 4 || PK || ENC(calldata)
	"0x000000000000000000000000000000045ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d521477814624e3acc6eca760f6e91dbee8fe70b5e07"
  );
}

/**
 * @returns the Counter contract's storage for the current count.
 */
async function _getCounterEnc(encryptedCalldata) {
  const params = [
	{
	  // counter contract address
	  "to": "0xf75d55dd51ee8756fbdb499cc1a963e702a52091",
	  "data": encryptedCalldata,
	},
	"latest"
  ];
  return makeRpc("confidential_call_enc", params);
}

/**
 * Sends a transaction to increase the contract's count.
 */
async function incrementCounterEnc() {
  const raw_tx = "0xf8a501808310000094f75d55dd51ee8756fbdb499cc1a963e702a5209180b844000000000000000000000000000000035ea9673a039960bd668120a8269933d74433d6e1e6df14a765f866503eb9d52153c65572cc810f0cc2f3a1568c8c9330e8b12c901ca053acbd540ed1bf74ca2f373f046910d9669c5e143d7bfc24cdbfb3f9aac60543a0613de408698598a9c3a4cf6a75a465236d8418bb42f2d24b23d3d0c8e18d745b";
  return makeRpc("confidential_sendRawTransaction", [raw_tx]);
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

  constructor(signerAddress, privateKey) {
	this.signerAddress = signerAddress;
	this.privateKey = privateKey;
	this.contractAddress = "";
	this.artifact = JSON.parse(fs.readFileSync("./build/contracts/Counter.json").toString());
	this.contract = new (new web3()).eth.Contract(this.artifact.abi);
  }

  async deploy() {
	const deployData = this.contract.deploy({data: this.artifact.bytecode}).encodeABI();
	const deployTx = new Tx({
	  data: deployData,
	  gasLimit: '0x100000',
	  from: this.signerAddress,
	  value: 0,
	  nonce: 0
	});
	deployTx.sign(this.privateKey);
	const rawTx = '0x' + deployTx.serialize().toString('hex');
	let txHash = await makeRpc("eth_sendRawTransaction", [rawTx]);
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
	const params = [
	  {
		"to": this.contractAddress,
		"data": this.contract.methods.getCounter().encodeABI(),
	  },
	  "latest"
	];
	return makeRpc("eth_call", params);
  }

  async incrementCounter() {
	let incrementCounterData = this.contract.methods.incrementCounter().encodeABI();
	const incrementTx = new Tx({
	  data: incrementCounterData,
	  gasLimit: '0x100000',
	  to: this.contractAddress,
	  from: this.signerAddress,
	  value: 0,
	  nonce: 1
	});
	incrementTx.sign(this.privateKey);
	let incrementRawTx = '0x' + incrementTx.serialize().toString('hex');
	return makeRpc("eth_sendRawTransaction", [incrementRawTx]);
  }
}
