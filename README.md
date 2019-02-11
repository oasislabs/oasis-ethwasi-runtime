# Ekiden Ethereum runtime

[![Build status](https://badge.buildkite.com/e1de50bd91d01f6aaf2b9fba113ad48b0118459d7d2c5dd2bd.svg)](https://buildkite.com/oasislabs/runtime-ethereum)
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
of the Ekiden binaries:
```bash
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-tools
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-worker
$ cargo install --git https://github.com/oasislabs/ekiden --branch master ekiden-keymanager-node
```

If you later need to update them to a new version use the `--force` flag to update.

You also need the Go node:
```bash
$ mkdir -p /go/src/github.com/oasislabs
$ cd /go/src/github.com/oasislabs
$ git clone https://github.com/oasislabs/ekiden
$ cd ekiden/go
$ make
$ cd ekiden
$ cp ekiden /go/bin/
```
**Note**: You need to be on Go v1.11 since this project uses [modules](https://github.com/golang/go/wiki/Modules), and the source must be built outside of the `GOPATH`. 

## Building the runtime

First build the keymanager enclave:
```bash
$ cd /go/src/github.com/oasislabs/ekiden/key-manager/dummy/enclave
$ cargo ekiden build-enclave --output-identity
```

This step is needed so that we can compile the keymanager's enclave identity statically into the runtime enclave upon initialization.

Then, to build the runtime run:
```bash
$ KM_ENCLAVE_PATH=<ekiden-keymanager-trusted.so path> cargo ekiden build-enclave --output-identity
```

The built enclave will be stored under `target/enclave/runtime-ethereum.so`.

## Building the web3 gateway

The web3 gateway is located under `gateway` and it may be built using:
```bash
$ cd gateway
$ cargo build
```

## Running

To start a validator committee, two compute nodes, and a single gateway running on port 8545:
```bash
$ ./scripts/gateway.sh
```

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
