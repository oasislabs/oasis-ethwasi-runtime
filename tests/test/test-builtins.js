const web3 = require("web3");
const Builtins = artifacts.require("./Builtins.sol");

/**
 * Tests EVM builtin precompiles using a readonly call and a transaction
 * for each test, exercising both the gateway and the compute nodes.
 */
contract("Builtins", async (accounts) => {

  let instance;
  const expectedSha256 = "0xdbc1b4c900ffe48d575b5da5c638040125f65db0fe3e24494b76ea986457d986";

  before(async () => {
    instance = await Builtins.new();
  });

  it("calls the sha256 method", async () => {
    const result = await instance._sha256.call("0x02");
    assert.equal(result, expectedSha256);
  });

  it("calls the sha256 event", async () => {
    const result = await instance._sha256Event("0x02");
    const hashResult = result.logs[0].args.hash;
    assert.equal(hashResult, expectedSha256);
  });

  const digest = "0xfdd52c7ab35a20a1a628083c3772887449019a8a7752668d61361321d8744e5c";
  const v = 28;
  const r = "0x65bdcfea25a0222307789118039dbd5f76924c936363c6d5d1bda9e44de4fdb3";
  const s = "0x569dac424272dc3193d9e900bf7f7ea08d4661a476c811b3d6c95affe599d873";
  const expectedAddress = "0x1E66891Ed84CEE8aF1942387aF83319440d82511";

  it("calls ecrecover", async () => {
    const result = await instance._ecrecover.call(digest, v, r, s);
    assert.equal(result, expectedAddress);
  });

  it("calls ecrecover event", async () => {
    const result = await instance._ecrecoverEvent(digest, v, r, s);
    const addrResult = result.logs[0].args.addr;
    assert.equal(addrResult, expectedAddress);
  });

  it("calls modexp", async () => {
	// n^0 mod z == 1.
	let result = await instance._modexp.call("0x012345", "0x00", "0x0789");
	assert.equal(web3.utils.hexToNumber(result), 1);
	// 0^m mod z == 0
	result = await instance._modexp.call("0x00", "0x0789", "0x078");
	assert.equal(web3.utils.hexToNumber(result), 0);
	// n^m mod 1 == 0
	result = await instance._modexp.call("0x012345", "0x0789", "0x01");
	assert.equal(web3.utils.hexToNumber(result), 0);
	// 2^10 mod 1000 == 24
	result = await instance._modexp.call("0x02", "0x0a", "0x03e8");
	assert.equal(web3.utils.hexToNumber(result), 24);
  });

  it ("calls modexp event", async () => {
	// n^0 mod z == 1.
	let result = await instance._modexpEvent("0x012345", "0x00", "0x0789");
	assert.equal(web3.utils.hexToNumber(result.logs[0].args.modexp), 1);
	// 0^m mod z == 0
	result = await instance._modexpEvent("0x00", "0x0789", "0x078");
	assert.equal(web3.utils.hexToNumber(result.logs[0].args.modexp), 0);
	// n^m mod 1 == 0
	result = await instance._modexpEvent("0x012345", "0x0789", "0x01");
	assert.equal(web3.utils.hexToNumber(result.logs[0].args.modexp), 0);
	// 2^10 mod 1000 == 24
	result = await instance._modexpEvent("0x02", "0x0a", "0x03e8");
	assert.equal(web3.utils.hexToNumber(result.logs[0].args.modexp), 24);
  });

});
