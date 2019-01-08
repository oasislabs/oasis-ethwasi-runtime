const BasicWasm = artifacts.require("BasicWasm")

const Web3 = require("web3");
const web3 = new Web3(BasicWasm.web3.currentProvider);

contract("BasicWasm", (accounts) => {

  it("should call a method of a basic wasm contract", async () => {
	const contract = new web3.eth.Contract(BasicWasm.abi);
	let instance = await contract.deploy({data: BasicWasm.bytecode}).send({
	  from: accounts[0]
	});
	const bytes = await instance.methods.my_method().call();
	assert.equal(bytes, "0x726573756c74");
  });

});
