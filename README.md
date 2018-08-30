# Ekiden Ethereum runtime

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

## Building the runtime

To build the runtime simply run:
```bash
$ cargo ekiden build-enclave
```

The built enclave will be stored under `target/enclave/runtime-ethereum.so`.

## Running the runtime

You need to run multiple Ekiden services, so it is recommended to run each of these in a
separate container shell, attached to the same container.

To start the shared dummy node:
```
$ ekiden-node-dummy \
    --random-beacon-backend dummy \
    --entity-ethereum-address 0000000000000000000000000000000000000000 \
    --time-source-notifier mockrpc \
    --storage-backend dummy
```

To start the compute node (you need to start at least two, on different ports):
```bash
$ ekiden-compute \
    --no-persist-identity \
    --storage-backend remote \
    --max-batch-timeout 10 \
    --time-source-notifier system \
    --entity-ethereum-address 0000000000000000000000000000000000000000 \
    --port <port number> \
    target/enclave/runtime-ethereum.so
```

After starting the nodes, to manually advance the epoch in the shared dummy node:
```
$ ekiden-node-dummy-controller set-epoch --epoch 1
```

The compute node will listen on `127.0.0.1` (loopback), TCP port `9001` by default.

Development notes:

* If you are changing things, be sure to either use the `--no-persist-identity` flag or remove the referenced enclave identity file (e.g., `/tmp/runtime-ethereum.identity.pb`). Otherwise the compute node will fail to start as it will be impossible to unseal the old identity.

## Building the web3 gateway

The web3 gateway is located under `gateway` and it may be built using:
```bash
$ cd gateway
$ cargo build
```

To run (in the same directory):
```bash
$ cargo run -- --storage-backend remote --mr-enclave <mr-enclave> --threads <number of threads for http server>
```

For `<mr-enclave>` you can use the value reported when starting the compute node.

## Benchmarking

To build the benchmarking version of the runtime (release build, logging suppressed, nonce checking disabled):
```bash
$ CARGO_TARGET_DIR=target_benchmark cargo ekiden build-enclave --output-identity --cargo-addendum feature.benchmark.addendum --target-dir target_benchmark --release -- --features "benchmark"
```

The built enclave will be stored under `target_benchmark/enclave/runtime-ethereum.so`.

Release builds of `benchmark`, `gateway`, `genesis`, and `playback` are also used for benchmarking. To build, for each component:
```bash
$ cd <component>
$ cargo build --release
```

Some sample benchmark driver scripts are located in `scripts/benchmarks/`.
