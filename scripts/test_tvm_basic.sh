#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

source scripts/utils.sh

# Ensure cleanup on exit.
# cleanup() is defined in scripts/utils.sh
trap 'cleanup' EXIT

export TVM_HOME=${TVM_HOME:-/tmp/tvm}

setup_tvm() {
    pushd $TVM_HOME

    # Install LLVM 8 with WASM support
    echo deb http://apt.llvm.org/xenial/ llvm-toolchain-xenial main \
        >> /etc/apt/sources.list.d/llvm.list && \
    wget -O - http://apt.llvm.org/llvm-snapshot.gpg.key|apt-key add - && \
        apt-get update && apt-get install -y llvm

    # Build TVM
    git clone --recursive https://github.com/dmlc/tvm
    mkdir build && cd build
    cmake .. -DUSE_LLVM=ON
    make -j4
    cd ..
    apt-get -y install python3-pip
    pip3 install -e $TVM_HOME/python -e $TVM_HOME/topi/python -e $TVM_HOME/nnvm/python
    popd
}

run_test() {
    CONTRACT_NAME="tvm_basic"

    echo "Building $CONTRACT_NAME."

    cp -r "$WORKDIR/tests/contracts/$CONTRACT_NAME" /tmp

    make -C /tmp/$CONTRACT_NAME

    run_dummy_node_go_tm
    sleep 1
    run_compute_node 1
    sleep 1
    run_compute_node 2
    sleep 1
    run_gateway 1
    sleep 10

    $WORKDIR/ekiden-node debug dummy set-epoch --epoch 1

    echo "Installing deploy_contract dependencies."
    cd $WORKDIR/tests/deploy_contract > /dev/null
    npm install > /dev/null
    npm install > /dev/null # continue installing once secp256k1 fails to install

    echo "Deploying and calling contract."
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

# setup_tvm
run_test
