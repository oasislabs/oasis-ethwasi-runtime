const Deployed = artifacts.require("./cross_contract/solidity/Deployed.sol")
const Existing = artifacts.require("./cross_contract/solidity/Existing.sol")
const DeployedRust = artifacts.require("DeployedRust");
const ExistingRust = artifacts.require("ExistingRust");

const Web3 = require("web3");
const web3 = new Web3(Deployed.web3.currentProvider);

contract("CrossContractCall", (accounts) => {

  let testCases = [
    [Deployed, Existing, "should update value in other solidity contract"],
    [DeployedRust, ExistingRust, "should update value in other rust contract"]
  ];

  testCases.forEach((test) => {
    it(test[2], async () => {
      let deployedArtifact = test[0];
      let existingArtifact = test[1];

      const deployedContract = new web3.eth.Contract(deployedArtifact.abi);
      let deployInstance = await deployedContract.deploy({
        data: deployedArtifact.bytecode
      }).send({
        from: accounts[0]
      });

      let prevA = await deployInstance.methods.a().call();
      assert.equal(prevA, 1, "Previous value is incorrect");

      const existingContract = new web3.eth.Contract(existingArtifact.abi, undefined, {
        from: accounts[0]
      });
      let existingInstance = await existingContract.deploy({
        data: existingArtifact.bytecode,
        arguments: [deployInstance.options.address]
      }).send();

      prevA = await existingInstance.methods.get_a().call();
      assert.equal(prevA, 1, "Previous value is incorrect");

      // remove gas param oncce this issue is addressed
      // https://github.com/oasislabs/runtime-ethereum/issues/478
      await existingInstance.methods.set_a(2).send({gas: '0x100000'});
      let newA = await deployInstance.methods.a().call();
      assert.equal(newA, 2, "Contract value was not updated")
    });
  });
});
