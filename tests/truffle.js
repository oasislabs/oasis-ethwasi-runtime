module.exports = {
  networks: {
    development: {
      host: "127.0.0.1",
      port: 8545,
      from: "0x7110316b618d20d0c44728ac2a3d683536ea682b",
      network_id: "*"
    },
    testnet: {
      host: "oasiscloud.io",
      port: 8545,
      from: "0x7110316b618d20d0c44728ac2a3d683536ea682b",
      network_id: "*"
    },
  }
};
