# Oasis runtime

[![Build status](https://badge.buildkite.com/e1de50bd91d01f6aaf2b9fba113ad48b0118459d7d2c5dd2bd.svg?branch=master)](https://buildkite.com/oasislabs/oasis-runtime)
[![Coverage Status](https://coveralls.io/repos/github/oasislabs/oasis-runtime/badge.svg?branch=master&t=shmqoK)](https://coveralls.io/github/oasislabs/oasis-runtime?branch=master)

## Contributing

See the [Ekiden Contributing Guidelines](https://github.com/oasislabs/ekiden/blob/master/CONTRIBUTING.md).

## Security

Read our [Security](https://github.com/oasislabs/ekiden/blob/master/SECURITY.md) document.

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

## Running the web3 gateway

To run a local Ekiden "cluster" and a development version of the web3 gateway, run:
```bash
$ make run-gateway
```
