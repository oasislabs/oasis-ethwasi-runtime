use bytes::Bytes;
use ethcore::client::{BlockId, StateOrBlock};
use ethcore::encoded;
use ethcore::error::CallError;
use ethcore::filter::Filter as EthcoreFilter;
use ethcore::header::BlockNumber;
use ethcore::state::backend::Basic as BasicBackend;
use ethereum_types::{Address, H256, U256};
use futures::future::Future;
use journaldb::overlaydb::OverlayDB;
use runtime_evm;
use rustc_hex::FromHex;

use evm_api::{Filter, Log, Receipt, Transaction, TransactionRequest};

use util::from_block_id;

type Backend = BasicBackend<OverlayDB>;

// record contract call success
macro_rules! contract_call_ok {
    ($ret:expr) => {{
        measure_counter_inc!("contract_call_succeeded");
        $ret
    }};
}

// record contract call failure
macro_rules! contract_call_error {
    ($call:expr, $e:ident, $ret:expr) => {{
        measure_counter_inc!("contract_call_failed");
        error!("{}: {:?}", $call, $e);
        $ret
    }};
}

pub struct Client {
    client: runtime_evm::Client,
}

impl Client {
    pub fn new(client: runtime_evm::Client) -> Self {
        Self { client: client }
    }

    /// block-related
    pub fn best_block_number(&self) -> BlockNumber {
        match self.client.get_block_height(false).wait() {
            Ok(height) => contract_call_ok!(height.into()),
            Err(e) => contract_call_error!("get_block_height", e, 0),
        }
    }

    pub fn block(&self, id: BlockId) -> Option<encoded::Block> {
        match self.client.get_block(from_block_id(id)).wait() {
            Ok(response) => contract_call_ok!(response.map(|block| encoded::Block::new(block))),
            Err(e) => contract_call_error!("get_block", e, None),
        }
    }

    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        if let BlockId::Hash(hash) = id {
            Some(hash)
        } else {
            match self.client.get_block_hash(from_block_id(id)).wait() {
                Ok(response) => contract_call_ok!(response),
                Err(e) => contract_call_error!("get_block_hash", e, None),
            }
        }
    }

    /// transaction-related
    pub fn transaction(&self, hash: H256) -> Option<Transaction> {
        match self.client.get_transaction(hash).wait() {
            Ok(response) => contract_call_ok!(response),
            Err(e) => contract_call_error!("get_transaction", e, None),
        }
    }

    pub fn transaction_receipt(&self, hash: H256) -> Option<Receipt> {
        match self.client.get_receipt(hash).wait() {
            Ok(response) => contract_call_ok!(response),
            Err(e) => contract_call_error!("get_receipt", e, None),
        }
    }

    pub fn logs(&self, filter: EthcoreFilter) -> Vec<Log> {
        let filter = Filter {
            from_block: from_block_id(filter.from_block),
            to_block: from_block_id(filter.to_block),
            address: match filter.address {
                Some(address) => Some(address.into_iter().map(Into::into).collect()),
                None => None,
            },
            topics: filter.topics.into_iter().map(Into::into).collect(),
            limit: filter.limit.map(Into::into),
        };
        match self.client.get_logs(filter).wait() {
            Ok(response) => contract_call_ok!(response),
            Err(e) => contract_call_error!("get_logs", e, vec![]),
        }
    }

    /// account state-related
    pub fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        match self.client.get_account_balance(*address).wait() {
            Ok(balance) => contract_call_ok!(Some(balance)),
            Err(e) => contract_call_error!("get_account_balance", e, None),
        }
    }

    pub fn code(&self, address: &Address, state: StateOrBlock) -> Option<Option<Bytes>> {
        // TODO: differentiate between no account vs no code?
        match self.client.get_account_code(*address).wait() {
            Ok(response) => contract_call_ok!(Some(response)),
            Err(e) => contract_call_error!("get_account_code", e, None),
        }
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        match self.client.get_account_nonce(*address).wait() {
            Ok(nonce) => contract_call_ok!(Some(nonce)),
            Err(e) => contract_call_error!("get_account_nonce", e, None),
        }
    }

    pub fn storage_at(
        &self,
        address: &Address,
        position: &H256,
        state: StateOrBlock,
    ) -> Option<H256> {
        match self.client.get_storage_at((*address, *position)).wait() {
            Ok(value) => contract_call_ok!(Some(value)),
            Err(e) => contract_call_error!("get_storage_at", e, None),
        }
    }

    /// evm-related
    pub fn call(&self, request: TransactionRequest) -> Result<Bytes, CallError> {
        match self.client.simulate_transaction(request).wait() {
            Ok(result) => Ok(result.result),
            Err(_e) => Err(CallError::Exceptional),
        }
    }

    pub fn estimate_gas(&self, request: TransactionRequest) -> Result<U256, CallError> {
        match self.client.simulate_transaction(request).wait() {
            Ok(result) => Ok(result.used_gas),
            Err(_e) => Err(CallError::Exceptional),
        }
    }

    pub fn send_raw_transaction(&self, raw: Bytes) -> Result<H256, CallError> {
        match self.client.execute_raw_transaction(raw).wait() {
            Ok(result) => Ok(result),
            Err(_e) => Err(CallError::Exceptional),
        }
    }
}
