var HDWalletProvider = require("truffle-hdwallet-provider");
var mnemonic = 'patient oppose cotton portion chair gentle jelly dice supply salmon blast priority';

module.exports = {
  // See <http://truffleframework.com/docs/advanced/configuration>
  // to customize your Truffle configuration!
  networks: {
    development: {
      provider: function () {
        return new HDWalletProvider(mnemonic, "http://localhost:8545/");
      },
      network_id: "*",
      gasPrice: 0
    },
    development2: {
      provider: function () {
        return new HDWalletProvider(mnemonic, "http://localhost:8546/");
      },
      network_id: "*",
      gasPrice: 0
    }
  },
  compilers: {
    external: {
      command: "./node_modules/.bin/oasis-compile",
      targets: [{
        path: "./.oasis-build/*.json"
      }]
    }
  }
};
