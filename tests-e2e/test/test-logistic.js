const Logistic = artifacts.require("Logistic")

const Web3 = require("web3");
const web3 = new Web3(Logistic.web3.currentProvider);

contract("Logistic", (accounts) => {

  it("should perform logistic regression", async () => {
	const contract = new web3.eth.Contract(Logistic.abi);
	let instance = await contract.deploy({data: Logistic.bytecode}).send({
	  from: accounts[0]
	});
	const bytes = await instance.methods.regression().call();
	assert.equal(bytes, "0x4d61746368696e6720636c617373657320697320313030");
  });

});
