# Oasis runtime

[![Build status](https://badge.buildkite.com/0b4e493086daa3fc34c604dfce6597c56da35cfd093bdd943d.svg?branch=master)](https://buildkite.com/oasislabs/oasis-runtime-ci)
[![Coverage Status](https://coveralls.io/repos/github/oasislabs/oasis-runtime/badge.svg?branch=master&t=shmqoK)](https://coveralls.io/github/oasislabs/oasis-runtime?branch=master)

## Contributing

See the [Oasis Core Contributing Guidelines](https://github.com/oasislabs/oasis-core/blob/master/CONTRIBUTING.md).

## Security

Read our [Security](https://github.com/oasislabs/oasis-core/blob/master/SECURITY.md) document.

## Setting up the development environment

First, make sure that you have everything required for Oasis Core installed by
following [the instructions](https://github.com/oasislabs/oasis-core/blob/master/README.md).

For building and running the runtime, you need to have Oasis Core artifacts available.
To do this, you can either:

* Build Oasis Core locally by checking out the oasis-core repository (e.g., in `/path/to/oasis-core`)
  and then running `OASIS_UNSAFE_SKIP_KM_POLICY=1 make -C /path/to/oasis-core`. After the
  process completes you can then run `make && make symlink-artifacts OASIS_CORE_SRC_PATH=/path/to/oasis-core`
  and all the required artifacts will be symlinked under `.oasis-core` and `.runtime`.

* Download Oasis Core artifacts from a release (for currently supported release see `OASIS_CORE_VERSION` file),
  and then set `OASIS_NODE=/path/to/oasis-node`, `OASIS_NET_RUNNER=/path/to/oasis-net-runner` and
  `OASIS_CORE_RUNTIME_LOADER=/path/to/oasis-core-runtime-loader` environment variables.

In the following instructions, the top-level directory is the directory
where the code has been checked out.

## Building the runtime

To build everything required for running the runtime, simply execute in the
top-level directory:
```bash
$ make
```

## Running the web3 gateway

To run a local Oasis network "cluster" and a development version of the web3 gateway, run:
```bash
$ make run-gateway
```

This command will launch a gateway with web3 RPC endpoints on ports 8545 (http) and 8555 (WebSocket).
For example,

```
curl -s \
    -X POST \
    http://127.0.0.1:8545 \
    -d @- \
    --header "Content-Type: application/json" \
    <<EOF
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "eth_getBalance",
  "params": [
    "0x1cca28600d7491365520b31b466f88647b9839ec",
    "latest"
  ]
}
EOF
```

Should give a result like
```
{"jsonrpc":"2.0","result":"0x56bc75e2d63100000","id":1}
```
