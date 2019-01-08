const Deployed = artifacts.require("./cross_contract/solidity/Deployed.sol")
const Existing = artifacts.require("./cross_contract/solidity/Existing.sol")
//const DeployedRust = artifacts.require("DeployedRust");

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
})
