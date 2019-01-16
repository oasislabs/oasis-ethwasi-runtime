################################
# Common functions for E2E tests
################################

# TODO: Share these with ekiden.

# Temporary test base directory.
TEST_BASE_DIR=$(mktemp -d --tmpdir ekiden-e2e-XXXXXXXXXX)

# Key manager variables shared between the compute node, gateway, and key manager
KM_KEY="${WORKDIR}/tests/keymanager/km.key"
KM_CERT="${WORKDIR}/tests/keymanager/km.pem"
KM_HOST="127.0.0.1"
KM_PORT="9003"
KM_MRENCLAVE=${WORKDIR}/target/enclave/ekiden-keymanager-trusted.mrenclave
KM_ENCLAVE=${WORKDIR}/target/enclave/ekiden-keymanager-trusted.so

EKIDEN_NODE=${WORKDIR}/ekiden-node
EKIDEN_WORKER=${WORKDIR}/ekiden-worker
KM_NODE=${WORKDIR}/ekiden-keymanager-node

# Run a Tendermint validator committee and a storage node.
#
# Sets EKIDEN_TM_GENESIS_FILE and EKIDEN_STORAGE_PORT.
run_backend_tendermint_committee() {
    local base_datadir=${TEST_BASE_DIR}/committee-data
    local validator_files=""
    let nodes=3

    # Provision the validators.
    for idx in $(seq 1 $nodes); do
        local datadir=${base_datadir}-${idx}
        rm -rf ${datadir}

        let port=(idx-1)+26656
        ${EKIDEN_NODE} \
            tendermint provision_validator \
            --datadir ${datadir} \
            --node_addr 127.0.0.1:${port} \
            --node_name ekiden-committee-node-${idx} \
            --validator_file ${datadir}/validator.json
        validator_files="$validator_files $datadir/validator.json"
    done

    # Create the genesis document.
    local genesis_file=${TEST_BASE_DIR}/genesis.json
    rm -Rf ${genesis_file}

    ${EKIDEN_NODE} \
        tendermint init_genesis \
        --genesis_file ${genesis_file} \
        ${validator_files}

    # Run the storage node.
    local storage_datadir=${TEST_BASE_DIR}/storage
    local storage_port=60000
    rm -Rf ${storage_datadir}

    ${EKIDEN_NODE} \
        storage node \
        --datadir ${storage_datadir} \
        --grpc.port ${storage_port} \
        --log.file ${TEST_BASE_DIR}/storage.log \
        &

    # Run the validator nodes.
    for idx in $(seq 1 $nodes); do
        local datadir=${base_datadir}-${idx}

        let grpc_port=(idx-1)+42261
        let tm_port=(idx-1)+26656

        ${EKIDEN_NODE} \
            --log.level debug \
            --log.file ${TEST_BASE_DIR}/validator-${idx}.log \
            --grpc.port ${grpc_port} \
            --grpc.log.verbose_debug \
            --epochtime.backend tendermint_mock \
            --beacon.backend tendermint \
            --metrics.mode none \
            --storage.backend client \
            --storage.client.address 127.0.0.1:${storage_port} \
            --scheduler.backend trivial \
            --registry.backend tendermint \
            --roothash.backend tendermint \
            --tendermint.core.genesis_file ${genesis_file} \
            --tendermint.core.listen_address tcp://0.0.0.0:${tm_port} \
            --tendermint.consensus.timeout_commit 250ms \
            --tendermint.log.debug \
            --datadir ${datadir} \
            &
    done

    # Export some variables so compute workers can find them.
    EKIDEN_STORAGE_PORT=${storage_port}
    EKIDEN_TM_GENESIS_FILE=${genesis_file}
}

# Run a compute node.
#
# Requires that EKIDEN_TM_GENESIS_FILE and EKIDEN_STORAGE_PORT are
# set. Exits with an error otherwise.
#
# Arguments:
#   id - compute node index
#
# Any additional arguments are passed to the Go node.
run_compute_node() {
    local id=$1
    shift
    local extra_args=$*

    # Ensure the genesis file and storage port are available.
    if [[ "${EKIDEN_TM_GENESIS_FILE:-}" == "" || "${EKIDEN_STORAGE_PORT:-}" == "" ]]; then
        echo "ERROR: Tendermint genesis and/or storage port file not configured. Did you use run_backend_tendermint_committee?"
        exit 1
    fi

    local data_dir=${TEST_BASE_DIR}/worker-$id
    rm -rf ${data_dir}
    local cache_dir=${TEST_BASE_DIR}/worker-cache-$id
    rm -rf ${cache_dir}
    local log_file=${TEST_BASE_DIR}/worker-$id.log

    # Generate port number.
    let grpc_port=id+10000
    let client_port=id+11000
    let p2p_port=id+12000
    let tm_port=id+13000

    ${EKIDEN_NODE} \
        --log.level debug \
        --grpc.port ${grpc_port} \
        --grpc.log.verbose_debug \
        --storage.backend client \
        --storage.client.address 127.0.0.1:${EKIDEN_STORAGE_PORT} \
        --epochtime.backend tendermint_mock \
        --beacon.backend tendermint \
        --metrics.mode none \
        --scheduler.backend trivial \
        --registry.backend tendermint \
        --roothash.backend tendermint \
        --tendermint.core.genesis_file ${EKIDEN_TM_GENESIS_FILE} \
        --tendermint.core.listen_address tcp://0.0.0.0:${tm_port} \
        --tendermint.consensus.timeout_commit 250ms \
        --tendermint.log.debug \
        --worker.backend sandboxed \
        --worker.binary ${EKIDEN_WORKER} \
        --worker.cache_dir ${cache_dir} \
        --worker.runtime.binary ${WORKDIR}/target/enclave/runtime-ethereum.so \
        --worker.runtime.id 0000000000000000000000000000000000000000000000000000000000000000 \
        --worker.client.port ${client_port} \
        --worker.p2p.port ${p2p_port} \
        --worker.leader.max_batch_timeout 100ms \
        --worker.key_manager.address ${KM_HOST}:${KM_PORT} \
        --worker.key_manager.certificate ${KM_CERT} \
        --datadir ${data_dir} \
        ${extra_args} 2>&1 | tee ${log_file} | sed "s/^/[compute-node-${id}] /" &
}

run_compute_committee() {
    args="--worker.runtime.replica_group_size 2 --worker.runtime.replica_group_backup_size 2"
    run_compute_node 1 $args
    sleep 1
    run_compute_node 2 $args
    sleep 1
    run_compute_node 3 $args
    sleep 1
    run_compute_node 4 $args

    # Wait for all nodes to register.
    ${EKIDEN_NODE} debug dummy wait-nodes --nodes 4
}

run_gateway() {
    local id=$1

    # Generate port numbers.
    let http_port=id+8544
    let ws_port=id+8554
    let prometheus_port=id+3000

    echo "Starting web3 gateway ${id} on ports ${http_port} and ${ws_port}."
    ${WORKDIR}/target/debug/gateway \
        --storage-backend multilayer \
        --storage-multilayer-local-storage-base ${TEST_BASE_DIR}/storage-persistent-gateway_${id} \
        --storage-multilayer-bottom-backend remote \
        --mr-enclave $(cat $WORKDIR/target/enclave/runtime-ethereum.mrenclave) \
        --test-runtime-id 0000000000000000000000000000000000000000000000000000000000000000 \
        --http-port ${http_port} \
        --threads 100 \
        --ws-port ${ws_port} \
        --key-manager-cert $KM_KEY \
        --key-manager-host $KM_HOST \
        --key-manager-port  $KM_PORT \
        --key-manager-mrenclave $(cat ${KM_MRENCLAVE}) \
        --prometheus-metrics-addr 0.0.0.0:${prometheus_port} \
        --prometheus-mode pull 2>&1 | tee ${TEST_BASE_DIR}/gateway-$id.log | sed "s/^/[gateway-${id}] /" &
}

run_keymanager_node() {
    local extra_args=$*

    local storage_dir=${TEST_BASE_DIR}/storage-persistent-keymanager
    rm -rf ${storage_dir}

    ${KM_NODE} \
        --enclave $KM_ENCLAVE \
        --node-key-pair $KM_KEY \
        --storage-backend dummy \
        --storage-path ${storage_dir} \
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
