#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

setup_tvm() {
    # Install LLVM 8 with WASM support
    echo deb http://apt.llvm.org/xenial/ llvm-toolchain-xenial main \
        >> /etc/apt/sources.list.d/llvm.list && \
    wget -O - http://apt.llvm.org/llvm-snapshot.gpg.key|apt-key add - && \
        apt-get update && apt-get install -y llvm

    llvm-config --version

    # Download TVM
    git clone --recursive https://github.com/dmlc/tvm /tmp/tvm

    # Build TVM
    pushd /tmp/tvm
    mkdir build
    cp cmake/config.cmake build
    sed -i 's/USE_LLVM OFF/USE_LLVM ON/' build/config.cmake

    # Install numpy and decorator
    apt-get update
    apt-get -y install python3-pip

    pip3 install numpy decorator

    pushd build
    cmake ..
    make -j4
    popd

    popd

    # Python Package Installation
    export TVM_HOME=/tmp/tvm
    export PYTHONPATH=$TVM_HOME/python:$TVM_HOME/topi/python:$TVM_HOME/nnvm/python:${PYTHONPATH}
}

run_test() {
    echo "Building contract."

    cp -r ${WORKDIR}/tests/contracts/tvm_basic_contract /tmp

    pushd /tmp/tvm_basic_contract
    make TARGET_DIR="${CARGO_TARGET_DIR:-target}"
    popd

    # Start dummy node.
    run_dummy_node_go_tm
    sleep 1

    # Start compute nodes.
    run_compute_node 1
    sleep 1
    run_compute_node 2

    run_gateway 1
    sleep 10

    ${WORKDIR}/ekiden-node debug dummy set-epoch --epoch 1

    echo "Installing deploy_contract dependencies."
    pushd ${WORKDIR}/tests/deploy_contract > /dev/null
    npm install > /dev/null
    npm install > /dev/null # continue installing once secp256k1 fails to install

    echo "Deploying and calling contract."
    echo $CARGO_TARGET_DIR
    ls $CARGO_TARGET_DIR
    OUTPUT="$(./deploy_contract.js --gas-limit 0xf42400 --gas-price 0x3b9aca00 $CARGO_TARGET_DIR/tvm_basic_contract.wasm)"
    echo "Contract address: $OUTPUT"
    OUTPUT="$(./call_contract.js $OUTPUT | tail -1)"
    echo "Fetched: $OUTPUT"

    if [ "$OUTPUT" = "0x73756363657373" ]; then
        echo "Test passed."
    else
        echo "Incorrect output. Expected 0x73756363657373."
        exit 1
    fi
}

setup_tvm
run_test
