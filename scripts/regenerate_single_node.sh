#!/bin/bash -e

DATADIR=$(mktemp -d --tmpdir ekiden-regenerate-XXXXXXXXXX)

EKIDEN_BINARY="${EKIDEN_ROOT_PATH}/go/ekiden/ekiden"
EKIDEN_RUNTIME_ID=${EKIDEN_RUNTIME_ID:-"0000000000000000000000000000000000000000000000000000000000000000"}
EKIDEN_KM_RUNTIME_ID=${EKIDEN_KM_RUNTIME_ID:-"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}

SINGLE_NODE_DIR=${SINGLE_NODE_DIR:-"./configs/single_node/"}
SINGLE_NODE_SGX_DIR=${SINGLE_NODE_SGX_DIR:-"./configs/single_node_sgx/"}

GENESIS_DIR=${GENESIS_DIR:-"./resources/genesis/"}

#
# Non-SGX config.
#

${EKIDEN_BINARY}\
    registry runtime init_genesis \
    --datadir ${DATADIR} \
    --debug.allow_test_keys \
    --debug.test_entity \
    --runtime.id ${EKIDEN_RUNTIME_ID} \
    --runtime.replica_group_size 1 \
    --runtime.replica_group_backup_size 0 \
    --runtime.storage_group_size 1 \
    --runtime.keymanager ${EKIDEN_KM_RUNTIME_ID} \
    --runtime.kind compute \
    --runtime.genesis.state ${GENESIS_DIR}/ekiden_genesis_testing.json \
    --runtime.genesis.file runtime_genesis_nosgx.json

${EKIDEN_BINARY} \
    registry runtime init_genesis \
    --datadir ${DATADIR} \
    --debug.allow_test_keys \
    --debug.test_entity \
    --runtime.id ${EKIDEN_KM_RUNTIME_ID} \
    --runtime.kind keymanager \
    --runtime.genesis.file keymanager_genesis_nosgx.json

${EKIDEN_BINARY} \
    genesis init \
    --datadir ${DATADIR} \
    --debug.allow_test_keys \
    --debug.test_entity \
    --storage ${GENESIS_DIR}/ekiden_genesis_testing.json \
    --genesis_file ${DATADIR}/genesis_nosgx.json \
    --runtime ${DATADIR}/keymanager_genesis_nosgx.json \
    --runtime ${DATADIR}/runtime_genesis_nosgx.json \
    --validator ${SINGLE_NODE_DIR}/validator-44f1c4b3a161a889e6876ba92c20c3f63dd1ecf204adab6ca436566497b01628.json

cp ${DATADIR}/genesis_nosgx.json ${SINGLE_NODE_DIR}/genesis.json

#
# SGX config.
#

${EKIDEN_BINARY}\
    registry runtime init_genesis \
    --datadir ${DATADIR} \
    --debug.allow_test_keys \
    --debug.test_entity \
    --runtime.id ${EKIDEN_RUNTIME_ID} \
    --runtime.replica_group_size 1 \
    --runtime.replica_group_backup_size 0 \
    --runtime.storage_group_size 1 \
    --runtime.keymanager ${EKIDEN_KM_RUNTIME_ID} \
    --runtime.kind compute \
    --runtime.genesis.state ${GENESIS_DIR}/ekiden_genesis_testing.json \
    --runtime.tee_hardware intel-sgx \
    --runtime.genesis.file runtime_genesis_sgx.json

${EKIDEN_BINARY} \
    registry runtime init_genesis \
    --datadir ${DATADIR} \
    --debug.allow_test_keys \
    --debug.test_entity \
    --runtime.id ${EKIDEN_KM_RUNTIME_ID} \
    --runtime.kind keymanager \
    --runtime.tee_hardware intel-sgx \
    --runtime.genesis.file keymanager_genesis_sgx.json

${EKIDEN_BINARY} \
    genesis init \
    --datadir ${DATADIR} \
    --debug.allow_test_keys \
    --debug.test_entity \
    --storage ${GENESIS_DIR}/ekiden_genesis_testing.json \
    --genesis_file ${DATADIR}/genesis_sgx.json \
    --runtime ${DATADIR}/keymanager_genesis_sgx.json \
    --runtime ${DATADIR}/runtime_genesis_sgx.json \
    --validator ${SINGLE_NODE_SGX_DIR}/validator-44f1c4b3a161a889e6876ba92c20c3f63dd1ecf204adab6ca436566497b01628.json

cp ${DATADIR}/genesis_sgx.json ${SINGLE_NODE_SGX_DIR}/genesis.json
