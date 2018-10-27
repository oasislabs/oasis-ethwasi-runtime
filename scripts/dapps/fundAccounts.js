const HDWalletProvider = require("truffle-hdwallet-provider");

/**
 * This script is used as a step prior to running our test suites
 * for various dapps, allocating wei from one account to another,
 * i.e., to the accounts used in our dapp test suites.
 *
 * The script should be invoked with three args as follows:
 *
 * node fundAccounts.js [OASIS_MNEMONIC] [DAPP_MNEMONIC] [NUM_ACCOUNTS] [TRANSFER_AMOUNT]
 *
 * OASIS_MNEMONIC  - mnemonic used to derive the account we'll transfer *from*
 * DAPP_MNEMONIC   - mnemonic used to derive the accounts we'll transfer *to*
 * NUM_ACCOUNTS    - number of accounts we'll fund in DAPP_MNEMONIC.
 * TRANSFER_AMOUNT - the amount of wei to transfer.
 */

async function fundAccounts() {
  const oasisMnemonic = process.argv[2];
  const dappMnemonic = process.argv[3];
  const numAccounts = process.argv[4];
  const transferAmount = process.argv[5];

  console.log(`Funding ${numAccounts} accounts with ${transferAmount} wei.\n\nFrom: ${oasisMnemonic}\nTo: ${dappMnemonic}\n`);

  const oasisProvider = new HDWalletProvider(oasisMnemonic, "http://localhost:8545");
  const dappProvider = new HDWalletProvider(dappMnemonic, "http://localhost:8545", 0, numAccounts);
  const web3 = new (require('web3'))(oasisProvider);

  for (let k = 0; k < numAccounts; k += 1) {
    await web3.eth.sendTransaction({
      from: oasisProvider.addresses[0],
      to: dappProvider.addresses[k],
      value: transferAmount,
    });
    console.log("Funded ", dappProvider.addresses[k]);
  }
  oasisProvider.engine.stop();
  dappProvider.engine.stop();
  console.log("\nFunding complete");
}

fundAccounts();
