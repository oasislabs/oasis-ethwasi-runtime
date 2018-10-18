const web3 = require("web3");
const utils = require("./utils");

describe("New accounts", async () => {

  const provider = utils.provider();

  it("transfers eth to an account that doesn't exist", async () => {
	let w3 = new web3(provider.provider);
	let account = w3.eth.accounts.create()

	const transferAmount = 100;

	const providerBeforeBalance = await w3.eth.getBalance(provider.address);
	const newAccountBeforeBalance = await w3.eth.getBalance(account.address);

	await w3.eth.sendTransaction({
	  from: provider.address,
	  to: account.address,
	  value: 100
	});

	const newAccountAfterBalance = await w3.eth.getBalance(account.address);
	const newAccountDiff = newAccountAfterBalance - newAccountBeforeBalance;

	assert.equal(newAccountDiff, transferAmount);
  });

});
