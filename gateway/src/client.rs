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

use ekiden_core::error::Error;
use evm_api::{Filter, Log, Receipt, Transaction, TransactionRequest};

use util::from_block_id;

type Backend = BasicBackend<OverlayDB>;

// record contract call outcome
fn contract_call_result<T>(call: &str, result: Result<T, Error>, default: T) -> T {
    match result {
        Ok(val) => {
            measure_counter_inc!("contract_call_succeeded");
            val
        }
        Err(e) => {
            measure_counter_inc!("contract_call_failed");
            error!("{}: {:?}", call, e);
            default
        }
    }
}

pub struct Client {
    client: runtime_evm::Client,
}

impl Client {
    pub fn new(client: runtime_evm::Client) -> Self {
        Self { client: client }
    }

    // block-related
    pub fn best_block_number(&self) -> BlockNumber {
        contract_call_result(
            "get_block_height",
            self.client.get_block_height(false).wait(),
            U256::from(0),
        ).into()
    }

    pub fn block(&self, id: BlockId) -> Option<encoded::Block> {
        contract_call_result::<Option<Vec<u8>>>(
            "get_block",
            self.client.get_block(from_block_id(id)).wait(),
            None,
        ).map(|block| encoded::Block::new(block))
    }

    pub fn block_hash(&self, id: BlockId) -> Option<H256> {
        if let BlockId::Hash(hash) = id {
            Some(hash)
        } else {
            contract_call_result(
                "get_block_hash",
                self.client.get_block_hash(from_block_id(id)).wait(),
                None,
            )
        }
    }

    // transaction-related
    pub fn transaction(&self, hash: H256) -> Option<Transaction> {
        contract_call_result(
            "get_transaction",
            self.client.get_transaction(hash).wait(),
            None,
        )
    }

    pub fn transaction_receipt(&self, hash: H256) -> Option<Receipt> {
        contract_call_result("get_receipt", self.client.get_receipt(hash).wait(), None)
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
        contract_call_result("get_logs", self.client.get_logs(filter).wait(), vec![])
    }

    // account state-related
    pub fn balance(&self, address: &Address, state: StateOrBlock) -> Option<U256> {
        contract_call_result(
            "get_account_balance",
            self.client.get_account_balance(*address).wait().map(Some),
            None,
        )
    }

    pub fn code(&self, address: &Address, state: StateOrBlock) -> Option<Option<Bytes>> {
        // TODO: differentiate between no account vs no code?
        contract_call_result(
            "get_account_code",
            self.client.get_account_code(*address).wait().map(Some),
            None,
        )
    }

    pub fn nonce(&self, address: &Address, id: BlockId) -> Option<U256> {
        contract_call_result(
            "get_account_nonce",
            self.client.get_account_nonce(*address).wait().map(Some),
            None,
        )
    }

    pub fn storage_at(
        &self,
        address: &Address,
        position: &H256,
        state: StateOrBlock,
    ) -> Option<H256> {
        contract_call_result(
            "get_storage_at",
            self.client
                .get_storage_at((*address, *position))
                .wait()
                .map(Some),
            None,
        )
    }

    // transaction-related
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
