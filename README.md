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
of the Ekiden compute node:
```bash
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-tools
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-compute
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-node-dummy
```

If you later need to update them to a new version use the `--force` flag to update.

## Building the EVM contract

To build the EVM contract simply run:
```bash
$ cargo ekiden build-contract
```

The built contract will be stored under `target/contract/evm.so`.

## Running the contract

You need to run multiple Ekiden services, so it is recommended to run each of these in a
separate container shell, attached to the same container.

To start the shared dummy node:
```
$ ekiden-node-dummy --time-source mockrpc
```

To start the compute node for the EVM contract (you need to start at least two):
```bash
$ ekiden-compute \
    --no-persist-identity \
    --max-batch-timeout 10 \
    target/contract/evm.so
```

After starting the nodes, to manually advance the epoch in the shared dummy node:
```
$ ekiden-node-dummy-controller set-epoch --epoch 1
```

The contract's compute node will listen on `127.0.0.1` (loopback), TCP port `9001` by default.

Development notes:

* If you are developing a contract and changing things, be sure to either use the `--no-persist-identity` flag or remove the referenced enclave identity file (e.g., `/tmp/evm.identity.pb`). Otherwise the compute node will fail to start as it will be impossible to unseal the old identity.

## Building the client

The web3 client is located under `client` and it may be built using:
```bash
$ cd client
$ cargo build
```

To run the client (in the same directory):
```bash
$ cargo run -- --mr-enclave <mr-enclave>
```

For `<mr-enclave>` you can use the value reported when starting the compute node.
