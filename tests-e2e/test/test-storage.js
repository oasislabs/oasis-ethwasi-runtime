const Storage = artifacts.require("Storage");

const Web3 = require("web3");
const web3 = new Web3(Storage.web3.currentProvider);

const utils = require("./utils");

contract("Storage", (accounts) => {

  it("Retrieve storage from the fetch_bytes interface", async () => {
	const contract = new web3.eth.Contract(Storage.abi);
	let instance = await contract.deploy({data: Storage.bytecode}).send({
	  from: accounts[0]
	});
	await utils.makeRpc("oasis_storeBytes", [[1, 2, 3, 4, 5], 9223372036854775807]);
	const bytes = await instance.methods.get().call();
	assert.equal(bytes, "0x0102030405");
  });

});
