//! Utility functions.
use ethcore::{
    executive::contract_address,
    transaction::LocalizedTransaction,
    types::{
        ids::BlockId,
        log_entry::{LocalizedLogEntry, LogEntry},
    },
};
use ethereum_types::Address;
use runtime_ethereum_api::{BlockId as EkidenBlockId, Log};

use crate::genesis;

pub fn lle_to_log(lle: LocalizedLogEntry) -> Log {
    Log {
        address: lle.entry.address,
        topics: lle.entry.topics.into_iter().map(Into::into).collect(),
        data: lle.entry.data.into(),
        block_hash: Some(lle.block_hash),
        block_number: Some(lle.block_number.into()),
        transaction_hash: Some(lle.transaction_hash),
        transaction_index: Some(lle.transaction_index.into()),
        log_index: Some(lle.log_index.into()),
        transaction_log_index: Some(lle.transaction_log_index.into()),
    }
}

pub fn le_to_log(le: LogEntry) -> Log {
    Log {
        address: le.address,
        topics: le.topics.into_iter().map(Into::into).collect(),
        data: le.data.into(),
        block_hash: None,
        block_number: None,
        transaction_hash: None,
        transaction_index: None,
        log_index: None,
        transaction_log_index: None,
    }
}

pub fn to_block_id(id: EkidenBlockId) -> BlockId {
    match id {
        EkidenBlockId::Number(number) => BlockId::Number(number.into()),
        EkidenBlockId::Hash(hash) => BlockId::Hash(hash),
        EkidenBlockId::Earliest => BlockId::Earliest,
        EkidenBlockId::Latest => BlockId::Latest,
    }
}

// pre-EIP86, contract addresses are calculated using the FromSenderAndNonce scheme
pub fn get_contract_address(sender: &Address, transaction: &LocalizedTransaction) -> Address {
    contract_address(
        genesis::SPEC
            .engine
            .create_address_scheme(transaction.block_number),
        sender,
        &transaction.nonce,
        &transaction.data,
    )
    .0
}
