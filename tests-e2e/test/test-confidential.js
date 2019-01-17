const Web3c = require("web3c");
const Tx = require("ethereumjs-tx");
const utils = require("./utils");

contract("Confidential Contracts", async (accounts) => {

  const provider = utils.provider();
  const web3c = new Web3c(provider.provider);
  const artifact = utils.readArtifact("Counter");
  // Timestamp is expected to be the the maximum u64, which is 18446744073709551615.
  // However, javascript represents all numbers as double precision floats with 52
  // bits of mantissa, and so one can only compare numbers in the safe zone, i.e.,
  // -(2^53 - 1) and 2^53 - 1, which is more than necessary to represent a unix
  // timestamp. We use the given timestamp, here, as it's javascript's representation
  // of 2^64-1 and thus conversion into it's less precise double precision.
  const expectedTimestamp = "18446744073709552000";

  let counterContract = new web3c.confidential.Contract(artifact.abi, undefined, {
	from: accounts[0]
  });

  it("stores the long term public key in the deploy logs", async () => {
	counterContract = await counterContract.deploy({data: artifact.bytecode})
	  .send()
	  .on('receipt', (receipt) => {
		assert.equal(Object.keys(receipt.events).length, 1);

		let log = receipt.events['0'];
		assert.equal(log.raw.topics[0], "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
		validatePublicKey(log.raw.data);
		assert.equal(log.transactionLogIndex, "0x0");
	  });
  });

  let publicKeyPayload = null;

  it("retrieves a public key with a max timestamp", async () => {
	publicKeyPayload = (await utils.makeRpc(
	  "confidential_getPublicKey",
	  [counterContract.options.address]
	)).result;
	assert.equal(publicKeyPayload.timestamp + "", expectedTimestamp);
	validatePublicKey(publicKeyPayload.public_key);
  });

  // Note we don't do validation of the signature here. See ekiden or web3c.js for
  // signature validation tests.
  it("retrieves a public key with a signature of the correct form", async () => {
	assert.equal(publicKeyPayload.signature.length, 130);
	assert.equal(publicKeyPayload.signature.substr(0, 2), '0x');
	assert.equal(/0x[a-z0-9]+/.test(publicKeyPayload.signature), true);
  });

  it("uses an auto incrementing nonce when encrypting many logs", async () => {
	// Ensure we overflow at least one byte in the counter.
	const numEvents = 256 + 1;
	// Execute a transaction to trigger a bunch of logs to be encrypted.
	const decryptedReceipt = await counterContract.methods.incrementCounterManyTimes(numEvents).send();
	// First check the decrypted data is as expected (web3c will decrypt  automatically).
	const events = decryptedReceipt.events.Incremented;
	assert.equal(events.length, numEvents);
	for (let k = 0; k < numEvents; k += 1) {
	  assert.equal(events[k].returnValues.newCounter, k+1);
	}
	// Now check all nonces are incremented by one.
	const txHash = decryptedReceipt.transactionHash;
	const encryptedReceipt = (await utils.makeRpc("eth_getTransactionReceipt", [txHash])).result;
	let last = undefined;
	encryptedReceipt.logs.forEach((log) => {
	  let nonce = utils.fromHexStr(log.data.substr(2, 32));
	  if (last == undefined) {
		last = nonce;
	  } else {
		let lastPlusOne = utils.incrementByteArray(last);
		assert.deepEqual(lastPlusOne, nonce);
	  }
	});
  });

});

/**
 * Check the key is there. Expect it to be any (unpredictable) key of the form
 * "0x9385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54".
 */
function validatePublicKey(publicKey) {
  assert.equal(publicKey.length, 66);
  assert.equal(publicKey.substr(0, 2), '0x');
  assert.equal(/0x[a-z0-9]+/.test(publicKey), true);
}
