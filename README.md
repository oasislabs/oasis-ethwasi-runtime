# Ekiden Ethereum runtime

[![Build status](https://badge.buildkite.com/e1de50bd91d01f6aaf2b9fba113ad48b0118459d7d2c5dd2bd.svg)](https://buildkite.com/oasislabs/runtime-ethereum)
[![Coverage Status](https://coveralls.io/repos/github/oasislabs/runtime-ethereum/badge.svg?branch=master&t=shmqoK)](https://coveralls.io/github/oasislabs/runtime-ethereum?branch=master)

## Setting up the development environment

First, make sure that you have everything required for Ekiden installed by
following [the Ekiden instructions](https://github.com/oasislabs/ekiden/blob/master/README.md).

For building and running the runtime, you need to have specific Ekiden artifacts available.
To do this, you can either:

* Build Ekiden locally by checking out the Ekiden repository (e.g., in `/path/to/ekiden`)
  and then running `EKIDEN_UNSAFE_SKIP_KM_POLICY=1 make -C /path/to/ekiden`. After the
  process completes you can then run `make && make symlink-artifacts EKIDEN_SRC_PATH=/path/to/ekiden`
  and all the required artifacts will be symlinked under `.ekiden` and `.runtime`.

* (Coming soon...) Download Ekiden artifacts from CI by running `make download-artifacts`. You need to have
  the correct `BUILDKITE_ACCESS_TOKEN` set up to do this.

* Manually provide the required artifacts in a custom directory and specify
  `EKIDEN_ROOT_PATH=/path/to/ekiden` on each invocation of `make`, e.g.
  `make EKIDEN_ROOT_PATH=/path/to/ekiden`.

In the following instructions, the top-level directory is the directory
where the code has been checked out.

## Building the runtime

To build everything required for running the runtime, simply execute in the
top-level directory:
```bash
$ make
```

## Running the gateway

To run a local single-node Ekiden "cluster" and a development version of the gateway, run:
```bash
$ make run-gateway
```

## Benchmarking

Benchmarks require smart contract compilation and deployment tools, so you need
to install:

* rust support for wasm32 target: `rustup target add wasm32-unknown-unknown`,

* wasm-build command: `cargo install owasm-utils-cli --bin wasm-build`,

* abigen command as part of go ethereum devtools:
```bash
go get -u github.com/ethereum/go-ethereum
cd $GOPATH/src/github.com/ethereum/go-ethereum/
make devtools
```
* xxd command for converting binary-compiled contract to readable hex format:
  `apt install xxd`,

To build the benchmarking version of the runtime (release build, logging suppressed, nonce checking disabled):
```bash
$ CARGO_TARGET_DIR=target_benchmark cargo build --release --features benchmark
```

Release builds of `gateway` and `genesis` are also used for benchmarking. To build, for each component:
```bash
$ cargo build -p <component> --release
```

Finally, to build the benchmarking go client with benchmarks and Rust smart
contracts:
```bash
$ cd benchmark
$ make
```

Run benchmarks by first spinning up the gateway (see previous chapter) and then
executing `benchmark/benchmark -b <benchmark_name>`. Benchmarks results
will be reported to STDOUT in JSON format.
