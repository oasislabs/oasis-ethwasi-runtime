# Ekiden Ethereum runtime

[![Build status](https://badge.buildkite.com/e1de50bd91d01f6aaf2b9fba113ad48b0118459d7d2c5dd2bd.svg)](https://buildkite.com/oasislabs/runtime-ethereum)
[![Coverage Status](https://coveralls.io/repos/github/oasislabs/runtime-ethereum/badge.svg?branch=master&t=shmqoK)](https://coveralls.io/github/oasislabs/runtime-ethereum?branch=master)

## Setting up the development environment

First, make sure that you have everything required for Ekiden installed by
following [the Ekiden instructions](https://github.com/oasislabs/ekiden/blob/master/README.md).

For building and running the runtime, you need to have specific Ekiden artifacts avaialable.
To do this, you can either:

* Build Ekiden locally by checking out the Ekiden repository (e.g., in `/path/to/ekiden`)
  and then running `make -C /path/to/ekiden`. After the process completes you can then
  run `make symlink-ekiden EKIDEN_SRC_PATH=/path/to/ekiden` and all the required artifacts
  will be symlinked under `.ekiden` so they will be used by `make` invocations.

* Download Ekiden artifacts from CI by running `make download-ekiden`. You need to have
  the correct `BUILDKITE_ACCESS_TOKEN` set up to do this.

* Manually provide the required artifacts in a custom directory and specify
  `EKIDEN_ROOT_PATH=/path/to/ekiden` on each invocation of `make`, e.g.
  `make EKIDEN_ROOT_PATH=/path/to/ekiden`.

In the following instructions, the top-level directory is the directory
where the code has been checked out.

## Building the runtime

To build everything required for running the runtime and benchmarks, simply execute in the
top-level directory:
```
$ make
```

## Running the gateway

To run a local single-node Ekiden "cluster" and a development version of the gateway, run:
```
$ make run-gateway
```

## Benchmarking

To build the benchmarking version of the runtime (release build, logging suppressed, nonce checking disabled):
```bash
$ CARGO_TARGET_DIR=target_benchmark cargo build --release --features benchmark
```

Release builds of `gateway` and `genesis` are also used for benchmarking. To build, for each component:
```bash
$ cargo build -p <component> --release
```
