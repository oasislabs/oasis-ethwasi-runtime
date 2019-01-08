const Deployed = artifacts.require("./cross_contract/solidity/Deployed.sol")
const Existing = artifacts.require("./cross_contract/solidity/Existing.sol")
//const DeployedRust = artifacts.require("DeployedRust");

const Web3 = require("web3");
const web3 = new Web3(Deployed.web3.currentProvider);

contract("CrossContractCall", (accounts) => {
  it("should update value in other contract", async () => {

	const deployedContract = new web3.eth.Contract(Deployed.abi);
	let deployInstance = await deployedContract.deploy({data: Deployed.bytecode}).send({
	  from: accounts[0]
	});

	let prevA = await deployInstance.methods.a().call();
	assert.equal(prevA, 1, "Previous value is incorrect");

	const existingContract = new web3.eth.Contract(Existing.abi, undefined, {from: accounts[0]});
	let existingInstance = await existingContract.deploy({
	  data: Existing.bytecode,
	  arguments: [deployInstance.options.address]
	}).send();

	await existingInstance.methods.setA(2).send();
	let newA = await deployInstance.methods.a().call();
    assert.equal(newA, 2, "Contract value was not updated")
  })

});

/*
contract("CrossContractCall", (accounts) => {
  it("should update value in other contract", async () => {
    let deployed = await Deployed.new()

    let prevA = await deployed.a()
    assert.equal(prevA.toNumber(), 1, "Previous value is incorrect")

    let existing = await Existing.new(deployed.address)
    await existing.setA(2)
    let newA = await deployed.a()
    assert.equal(newA.toNumber(), 2, "Contract value was not updated")
  })
  /*
  it("should update value in other rust contract", async () => {
	let deployed = await DeployedRust.new();
	console.log("deployed = ", deployed);
  });*/
//})
