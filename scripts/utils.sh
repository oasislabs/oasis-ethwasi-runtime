#!/bin/bash -e

WORKDIR=${1:-$(pwd)}

# Key manager variables shared between the compute node, gateway, and key manager
KM_CERT="/tmp/km.key"
KM_HOST="127.0.0.1"
KM_PORT="9003"
KM_MRENCLAVE=${WORKDIR}/target/enclave/ekiden-keymanager-trusted.mrenclave
KM_ENCLAVE=${WORKDIR}/target/enclave/ekiden-keymanager-trusted.so

run_dummy_node_go_tm() {
    local datadir=/tmp/ekiden-dummy-data
    rm -rf ${datadir}

    echo "Starting Go dummy node."

    ${WORKDIR}/ekiden-node \
        --log.level debug \
        --grpc.port 42261 \
        --epochtime.backend tendermint_mock \
        --beacon.backend insecure \
        --storage.backend memory \
        --scheduler.backend trivial \
        --registry.backend tendermint \
        --roothash.backend tendermint \
        --tendermint.consensus.timeout_commit 250ms \
        --datadir ${datadir} \
        &> dummy-go.log &
}

run_compute_node() {
    local id=$1
    shift
    local extra_args=$*

    local cache_dir=/tmp/ekiden-test-worker-cache-$id
    rm -rf ${cache_dir}

    # Generate port number.
    let "port=id + 10000"

    echo "Starting compute node ${id} on port ${port}."

    ${WORKDIR}/ekiden-compute \
        --worker-path ${WORKDIR}/ekiden-worker \
        --worker-cache-dir ${cache_dir} \
        --no-persist-identity \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent_${id} \
        --storage-multilayer-bottom-backend remote \
        --max-batch-timeout 100 \
        --entity-ethereum-address 0000000000000000000000000000000000000000 \
        --key-manager-cert $KM_CERT \
        --key-manager-host $KM_HOST \
        --key-manager-port $KM_PORT \
        --port ${port} \
        ${extra_args} \
        ${WORKDIR}/target/enclave/runtime-ethereum.so &> compute${id}.log &
}

run_gateway() {
    local id=$1

    # Generate port numbers.
    let "http_port=id + 8544"
    let "ws_port=id + 8554"
    let "prometheus_port=id + 3000"

    echo "Starting web3 gateway ${id} on ports ${http_port} and ${ws_port}."
    target/debug/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base /tmp/ekiden-storage-persistent-gateway_${id} \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --http-port ${http_port} \
        --threads 100 \
        --ws-port ${ws_port} \
        --key-manager-cert $KM_CERT \
        --key-manager-host $KM_HOST \
        --key-manager-port  $KM_PORT \
        --key-manager-mrenclave $(cat ${KM_MRENCLAVE}) \
        --prometheus-metrics-addr 0.0.0.0:${prometheus_port} \
        --prometheus-mode pull &> gateway${id}.log &
}

run_keymanager_node() {
    local extra_args=$*

    ${WORKDIR}/ekiden-keymanager-node \
        --enclave $KM_ENCLAVE \
        --node-key-pair $KM_CERT \
        --storage-backend dummy \
        ${extra_args} &
}

##
# A useful function for printing debug statements
# that will prepend the name of the calling script
# before echoing all provided arguments.
#
# Instead of:
# echo "something"  # will output "something"
#
# You can use:
# debug "something" # will output "[some_script_name.sh] something"
##
debug() {
  script_name=$(basename $0)
  echo "[$script_name] $@"
}


##
# This function cleans up all child processes
# of the calling process before exiting with
# the status code of the previous command before
# this cleanup function was called.
#
# Common use of this function to ensure child
# processes are terminated is:
# trap 'cleanup' EXIT
##
cleanup() {
  prev_exit_code=$?
  debug "Previous exit code: $prev_exit_code"

  # Send all child processes a kill signal.
  # NOTE: This uses some bash trickery. The pkill
  # command returns an exit code of 1 if one or more
  # processes match the criteria. This causes the script
  # to exit immediately because we are running with -e.
  # So, we use '$$ true' here because '-e' will not kill
  # the script if a non-zerp status code is returned
  # as part of an && list. See these docs:
  # 1) https://linux.die.net/man/1/pkill
  # 2) https://ss64.com/bash/set.html
  debug " Cleaning up child processes."
  pkill -P $$ && true

  wait

  debug "Cleanup complete."

  if [[ $prev_exit_code != 0 ]]; then
    debug "Exiting with exit code of previous command."
    exit $prev_exit_code
  else
    debug "Exiting with status code 0."
    exit 0;
  fi
}

##
# Returns the current path to which cargo
# installs binaries during 'cargo install'.
# This can be useful with you want to verify
# whether a binary is already installed before
# calling 'cargo install', since the 'cargo install'
# command is not idempotent and will error out
# if the binary is already installed.
#
# Making scripts idempotent is really useful for
# local development and testing. Check the
# 'cargo install --help' docs for precedence rules
# for how installation root is determined.
##
get_cargo_install_root() {
  if [ -n "$CARGO_INSTALL_ROOT" ]; then
    cargo_install_root=${CARGO_INSTALL_ROOT}
  elif [ -n "$CARGO_HOME" ]; then
    cargo_install_root=${CARGO_HOME}
  else
    cargo_install_root=${HOME/.cargo}
  fi
  echo $cargo_install_root
}
