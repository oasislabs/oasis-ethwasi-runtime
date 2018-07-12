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

pub struct Client {
    client: runtime_evm::Client,
}

impl Client {
    pub fn new(client: runtime_evm::Client) -> Self {
        Self { client: client }
    }

    /// block-related
    pub fn best_block_number(&self) -> BlockNumber {
        let block_height = self.client.get_block_height(false).wait().unwrap();
        block_height.into()
    }

    pub fn block(&self, id: BlockId) -> Option<encoded::Block> {
        let response = self.client.get_block(from_block_id(id)).wait().unwrap();
        match response {
            Some(block) => Some(encoded::Block::new(block)),
            None => None,
        }
    }

    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        let response = if let BlockId::Hash(hash) = id {
            Some(hash)
        } else {
            self.client
                .get_block_hash(from_block_id(id))
                .wait()
                .unwrap()
        };
        response.map(Into::into)
    }

    /// transaction-related
    pub fn transaction(&self, hash: H256) -> Option<Transaction> {
        self.client.get_transaction(hash).wait().unwrap()
    }

    pub fn transaction_receipt(&self, hash: H256) -> Option<Receipt> {
        self.client.get_receipt(hash).wait().unwrap()
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
        self.client.get_logs(filter).wait().unwrap()
    }

    /// account state-related
    pub fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        let balance = self.client.get_account_balance(*address).wait().unwrap();
        Some(balance)
    }

    pub fn code(&self, address: &Address, state: StateOrBlock) -> Option<Option<Bytes>> {
        // TODO: differentiate between no account vs no code?
        let code = self.client.get_account_code(*address).wait().unwrap();
        match FromHex::from_hex(code.as_str()) {
            Ok(bytes) => Some(Some(bytes)),
            Err(_) => Some(None),
        }
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        let nonce = self.client.get_account_nonce(*address).wait().unwrap();
        Some(nonce)
    }

    pub fn storage_at(
        &self,
        address: &Address,
        position: &H256,
        state: StateOrBlock,
    ) -> Option<H256> {
        let value = self.client
            .get_storage_at((*address, *position))
            .wait()
            .unwrap();
        Some(value)
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
