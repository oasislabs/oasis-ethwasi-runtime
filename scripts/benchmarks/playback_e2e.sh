#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

# Paths to Go node and keymanager enclave, assuming they were built according to the README
: ${EKIDEN_NODE:=/go/src/github.com/oasislabs/ekiden/go/ekiden/ekiden}
: ${KM_MRENCLAVE:=/go/src/github.com/oasislabs/ekiden/target/enclave/ekiden-keymanager-trusted.mrenclave}
: ${KM_ENCLAVE:=/go/src/github.com/oasislabs/ekiden/target/enclave/ekiden-keymanager-trusted.so}

# Paths to ekiden binaries
: ${EKIDEN_WORKER:=$(which ekiden-worker)}
: ${KM_NODE:=$(which ekiden-keymanager-node)}

# Path to benchmark client
: ${BENCHMARK_CLIENT:=benchmark/benchmark}
: ${GATEWAY:=${WORKDIR}/target/release/gateway}
: ${BLOCKS:=${WORKDIR}/resources/benchmark/blocks.bin}

if [ ! -f ${BLOCKS} ]; then
    echo -e "\nFatal: File ${BLOCKS} does not contain valid parity binary dump.\n"
    exit -1
fi

source ${SCRIPTS_UTILS:-scripts/utils.sh}

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

run_test() {
    # Start benchmark.
    echo "Starting benchmark using ${BLOCKS}."
    ${BENCHMARK_CLIENT} \
        --benchmarks.concurrency 100 \
        --benchmarks playback \
        --benchmarks.playback.dataset ${BLOCKS} \
        --benchmarks.playback.transactions 10000000 \
        --log.level INFO
}

run_test_network
run_test
cleanup
