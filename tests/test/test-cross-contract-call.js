const Deployed = artifacts.require("./cross_contract/Deployed.sol")
const Existing = artifacts.require("./cross_contract/Existing.sol")

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
})
