#! /bin/bash

#############################################
# Simple wrapper script to trigger benchmark
# job, currently still on circle with the
# correct arguments.
#
# This script is intended to have buildkite
# specific things, like env vars and calling
# the buildkite-agent binary.
##############################################

# Helpful tips on writing build scripts:
# https://buildkite.com/docs/pipelines/writing-build-scripts
set -euxo pipefail

# Benchmark image
benchmark_image=oasislabs/ekiden-runtime-ethereum:benchmarking-latest

# Extract MRENCLAVE from the benchmark image
mr_enclave=$(
    docker run \
        --rm \
        --entrypoint=cat \
        ${benchmark_image} \
        /ekiden/res/runtime-ethereum-benchmarking.mrenclave
)

circleci_secret_token=$(cat ~/.circleci/private_ops_api_token)

curl -f \
    -H "Content-Type: application/json" \
    -d '{
        "revision": "master",
        "build_parameters": {
        "CIRCLE_JOB": "benchmark-runtime-ethereum",
        "BENCHMARK_IMAGE": "'"${benchmark_image}"'",
        "BENCHMARK_BUILD_NUM": "'"${BUILDKITE_BUILD_NUMBER}"'",
        "BENCHMARK_MR_ENCLAVE": "'"${mr_enclave}"'"
        }
    }' \
    "https://circleci.com/api/v1.1/project/github/oasislabs/private-ops/tree/master?circle-token=${circleci_secret_token}"
