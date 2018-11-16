# Ekiden Ethereum runtime

[![Coverage Status](https://coveralls.io/repos/github/oasislabs/runtime-ethereum/badge.svg?branch=master&t=shmqoK)](https://coveralls.io/github/oasislabs/runtime-ethereum?branch=master)

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
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-worker
```

If you later need to update them to a new version use the `--force` flag to update.

You also need the Go dummy node:
```bash
$ mkdir -p /go/src/github.com/oasislabs
$ cd /go/src/github.com/oasislabs
$ git clone https://github.com/oasislabs/ekiden
$ cd ekiden/go
$ make
$ cd ekiden
$ go install
```

## Building the runtime

To build the runtime simply run:
```bash
$ cargo ekiden build-enclave --output-identity
```

The built enclave will be stored under `target/enclave/runtime-ethereum.so`.

## Building the web3 gateway

The web3 gateway is located under `gateway` and it may be built using:
```bash
$ cd gateway
$ cargo build
```

## Running

*Easy mode*:

To start the shared dummy node, two compute nodes, and a single gateway running on port 8545:
```bash
$ ./scripts/gateway.sh
```

*Hard mode*:

You need to run multiple Ekiden services, so it is recommended to run each of these in a
separate container shell, attached to the same container.

To start the shared dummy node:
```bash
$ ekiden \
    --log.level debug \
    --grpc.port 42261 \
    --epochtime.backend tendermint \
    --epochtime.tendermint.interval 30 \
    --beacon.backend tendermint \
    --storage.backend memory \
    --scheduler.backend trivial \
    --registry.backend tendermint \
    --roothash.backend tendermint \
    --datadir /tmp/ekiden-dummy-data
```

To start the compute node (you need to start at least two, on different ports):
```bash
$ ekiden-compute \
    --worker-path $(which ekiden-worker) \
    --worker-cache-dir <cache directory, e.g., /tmp/ekiden-worker-cache-id> \
    --no-persist-identity \
    --storage-backend multilayer \
    --storage-multilayer-local-storage-base <storage directory, e.g., /tmp/ekiden-storage-id> \
    --storage-multilayer-bottom-backend remote \
    --max-batch-timeout 100 \
    --entity-ethereum-address 0000000000000000000000000000000000000000 \
    --port <port number> \
    target/enclave/runtime-ethereum.so
```

The compute node will listen on `127.0.0.1` (loopback), TCP port `9001` by default.

To start the gateway:
```bash
$ target/debug/gateway \
    --storage-backend multilayer \
    --storage-multilayer-local-storage-base /tmp/ekiden-storage-gateway \
    --storage-multilayer-bottom-backend remote \
    --mr-enclave <mr-enclave> \
    --threads <number of threads for http server>
```

For `<mr-enclave>` you can use the value reported when starting the compute node.

Development notes:

* If you are changing things, be sure to either use the `--no-persist-identity` flag or remove the referenced enclave identity file (e.g., `/tmp/runtime-ethereum.identity.pb`). Otherwise the compute node will fail to start as it will be impossible to unseal the old identity.

## Benchmarking

To build the benchmarking version of the runtime (release build, logging suppressed, nonce checking disabled):
```bash
$ CARGO_TARGET_DIR=target_benchmark cargo ekiden build-enclave --output-identity --cargo-addendum feature.benchmark.addendum --target-dir target_benchmark --release -- --features "benchmark"
```

The built enclave will be stored under `target_benchmark/enclave/runtime-ethereum.so`.

Release builds of `gateway` and `genesis` are also used for benchmarking. To build, for each component:
```bash
$ cd <component>
$ cargo build --release
```

The actual benchmark itself is written in Go.  To build the benchmark:
```bash
$ cd benchmark
$ make
```

Some sample benchmark driver scripts are located in `scripts/benchmarks/`.
