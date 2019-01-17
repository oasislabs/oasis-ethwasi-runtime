const Deployed = artifacts.require("./cross_contract/solidity/Deployed.sol")
const Existing = artifacts.require("./cross_contract/solidity/Existing.sol")
const DeployedRust = artifacts.require("DeployedRust");
const ExistingRust = artifacts.require("ExistingRust");

contract("CrossContractCall", (accounts) => {

  let testCases = [
    [Deployed, Existing, "should update value in other solidity contract"],
    [DeployedRust, ExistingRust, "should update value in other rust contract"]
  ];

  testCases.forEach((test) => {
    it(test[2], async () => {
      let deployedArtifact = test[0]
      let existingArtifact = test[1]

      let deployed = await deployedArtifact.new()
      let prevA = await deployed.a()
      assert.equal(prevA.toNumber(), 1, "Previous value is incorrect")

      let existing = await existingArtifact.new(deployed.address)

      prevA = await existing.get_a();
      assert.equal(prevA.toNumber(), 1, "Previous value is incorrect")

      await existing.set_a(2)
      let newA = await deployed.a()
      assert.equal(newA.toNumber(), 2, "Contract value was not updated")
    })
  })
})
