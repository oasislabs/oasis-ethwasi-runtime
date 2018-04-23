# EVM Ekiden contract

## Setting up the development environment

The easiest way to build SGX code is to use the provided ekiden "shell,"  which runs a Docker
container with all the included tools. Follow instructions for installing ekiden-tools provided in the [Ekiden repository](https://github.com/oasislabs/ekiden).

To start the SGX development container:
```bash
$ cargo ekiden shell
```

All the following commands should be run in the container and not on the host.

## Configuring repository authentication

Until all Ekiden repositories are public, you need to configure your Git inside the container
to correctly authenticate against GitHub. The best way is to generate a personal authentication
token on GitHub and use it as follows inside the container:
```bash
$ git config --global credential.helper store
$ git config --global credential.https://github.com.username <username>
$ echo "https://<username>:<token>@github.com" > ~/.git-credentials
```

## Installing tools

*In the future, these will already be part of the development container.*

You should install the correct versions (e.g., the same that you build against in `Cargo.toml`)
of the Ekiden compute and consensus nodes:
```bash
$ cargo install --git https://github.com/oasislabs/ekiden --tag 0.1.0-alpha.3 ekiden-tools
$ cargo install --git https://github.com/oasislabs/ekiden --tag 0.1.0-alpha.3 ekiden-compute
$ cargo install --git https://github.com/oasislabs/ekiden --tag 0.1.0-alpha.3 ekiden-consensus
```

If you later need to update them to a new version use the `--force` flag to update.

## Building the key manager contract

Before you can build your contract, you need to choose a key manager contract to manage
keys for your contract's state. A key manager contract is provided with Ekiden core in
the `ekiden-key-manager` crate.

To build it:
```bash
$ cargo ekiden build-contract \
    --git https://github.com/oasislabs/ekiden \
    --tag 0.1.0-alpha.3 \
    --output target/contract \
    ekiden-key-manager
```

## Building the EVM contract

To build the EVM contract simply run:
```bash
$ cargo ekiden build-contract
```

The built contract will be stored under `target/contract/evm.so`.

## Running the contract

You need to run multiple Ekiden services, so it is recommended to run each of these in a
separate container shell, attached to the same container.

To start the dummy consensus node:
```bash
$ ekiden-consensus -x
```

The `-x` flag tells the consensus node to not depend on Tendermint.

To start the compute node for the key manager contract:
```bash
$ ekiden-compute \
    -p 9003 \
    --disable-key-manager \
    --identity-file /tmp/key-manager.identity.pb \
    target/contract/ekiden-key-manager.so
```

To start the compute node for the EVM contract:
```bash
$ ekiden-compute \
    --identity-file /tmp/evm.identity.pb \
    target/contract/evm.so
```

The contract's compute node will listen on `127.0.0.1` (loopback), TCP port `9001` by default.

Development notes:

* If you are developing a contract and changing things, be sure to remove the referenced identity file (e.g., `/tmp/evm.identity.pb`) as it will otherwise fail to start as it will be impossible to unseal the old identity.
* Also, when the contract hash changes, the contract will be unable to decrypt and old state as the key manager will give it fresh keys. So be sure to also clear (if you are using a Tendermint node) and restart the consensus node.

## Building the example client

The example client is located under `examples/client` and it may be built using:
```bash
$ cd examples/client
$ cargo build
```

To run the client (in the same directory):
```bash
$ cargo run -- --mr-enclave <mr-enclave>
```

For `<mr-enclave>` you can use the value reported when starting the compute node.
