# Ekiden Ethereum runtime

[![Build status](https://badge.buildkite.com/e1de50bd91d01f6aaf2b9fba113ad48b0118459d7d2c5dd2bd.svg)](https://buildkite.com/oasislabs/runtime-ethereum)
[![Coverage Status](https://coveralls.io/repos/github/oasislabs/runtime-ethereum/badge.svg?branch=master&t=shmqoK)](https://coveralls.io/github/oasislabs/runtime-ethereum?branch=master)

## Setting up the development environment

The easiest way to build SGX code is to use the provided ekiden "shell,"  which runs a Docker
container with all the included tools. Follow instructions for installing ekiden-tools provided in the [Ekiden repository](https://github.com/oasislabs/ekiden).

To start the SGX development container:
```bash
cargo ekiden shell
```

All the following commands should be run in the container and not on the host.

## Configuring repository authentication

Until all Ekiden repositories are public, you need to configure your Git inside the container
to correctly authenticate against GitHub. The best way is to generate a personal authentication
token on GitHub and use it as follows inside the container:
```bash
git config --global credential.helper store
git config --global credential.https://github.com.username <username>
echo "https://<username>:<token>@github.com" > ~/.git-credentials
```

## Installing tools

```bash
export EKIDEN_HOME=/ekiden
git clone https://github.com/oasislabs/ekiden $EKIDEN_HOME
cd $EKIDEN_HOME
make
export PATH=$EKIDEN_HOME/go/ekiden:$PATH
```
**Note**: You need to be on Go v1.11 since this project uses [modules](https://github.com/golang/go/wiki/Modules), and the source must be built outside of the `GOPATH`.

## Building the runtime

```bash
cargo ekiden build-enclave --output-identity
```

The runtime enclave will be stored under `target/enclave/runtime-ethereum.so`.

## Building the web3 gateway

The web3 gateway is located under `gateway` and it may be built using:
```bash
cd gateway && CARGO_TARGET_DIR=target-dir cargo build && cd -
```

Note: the environment variable `CARGO_TARGET_DIR` is not necessary in order to compile the project, but when using docker as a build environment, pointing target-dir to a path that is not shared with a mounted volume improves significantly the build times. For example if the path `/code` is mounted on docker, setting `CARGO_TARGET_DIR=/target` should do the trick.

## Running

To start a validator committee, two compute nodes, and a single gateway running on port 8545:
```bash
scripts/gateway.sh
```

## Benchmarking

To build the benchmarking version of the runtime (release build, logging suppressed, nonce checking disabled):
```bash
$ CARGO_TARGET_DIR=target_benchmark cargo ekiden build-enclave --output-identity --cargo-addendum feature.benchmark.addendum --target-dir target_benchmark --release -- --features "benchmark"
```

The built enclave will be stored under `target_benchmark/enclave/runtime-ethereum.so`.

Release builds of `gateway` and `genesis` are also used for benchmarking. To build, for each component:
```bash
cd <component> && cargo build --release
```

The actual benchmark itself is written in Go.  To build the benchmark:
```bash
cd benchmark && make
```

Some sample benchmark driver scripts are located in `scripts/benchmarks/`.
