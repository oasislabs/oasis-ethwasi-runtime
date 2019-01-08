const web3c = require("web3c");
const Tx = require("ethereumjs-tx");
const utils = require("./utils");

contract("Confidential Contracts", async (accounts) => {

  const provider = utils.provider();

  it("stores the long term public key in the deploy logs", async () => {
	const artifact = utils.readArtifact("Counter");
	const counterContract = new (new web3c(provider.provider)).confidential.Contract(artifact.abi);
	await counterContract.deploy({data: artifact.bytecode})
	  .send({ from: accounts[0] })
	  .on('receipt', (receipt) => {
		assert.equal(Object.keys(receipt.events).length, 1);

		let log = receipt.events['0'];
		assert.equal(log.raw.topics[0], "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
		// Check the key is there. Expect it to be any (unpredictable) key of the form
		// "0x9385b8391e06d67c3de1675a58cffc3ad16bcf7cc56ab35d7db1fc03fb227a54";
		assert.equal(log.raw.data.length, 66);
		assert.equal(log.raw.data.substr(0, 2), '0x');
		assert.equal(/0x[a-z0-9]+/.test(log.raw.data), true);
		assert.equal(log.transactionLogIndex, "0x0");
	  });
  });
});
