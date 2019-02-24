#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Paths to Go node and keymanager enclave, assuming they were built according to the README
EKIDEN_HOME=${EKIDEN_HOME:-/go/src/github.com/oasislabs/ekiden}
EKIDEN_NODE=$EKIDEN_HOME/go/ekiden/ekiden
KM_MRENCLAVE=$EKIDEN_HOME/target/enclave/ekiden-keymanager-trusted.mrenclave
KM_ENCLAVE=$EKIDEN_HOME/target/enclave/ekiden-keymanager-trusted.so

# Paths to ekiden binaries
EKIDEN_WORKER=$(which ekiden-worker)
KM_NODE=$(which ekiden-keymanager-node)

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT
run_test_network
wait
