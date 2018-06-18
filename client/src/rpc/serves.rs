use super::{
  filter::*, serialize::*, util::*, DebugRPC, Either, EthereumRPC, FilterRPC, RPCBlock,
  RPCBlockTrace, RPCDump, RPCLog, RPCLogFilter, RPCReceipt, RPCTrace, RPCTraceConfig,
  RPCTransaction,
};

use error::Error;

use ethereum_types::{Address, H256, U256};
use evm_api::{error::INVALID_BLOCK_NUMBER, BlockRequest};
use std::{
  str::FromStr,
  sync::{Arc, Mutex},
};

use jsonrpc_macros::Trailing;

use ekiden_rpc_client;
use evm;
use futures::future::Future;

use hex;

use log::{info, log};

pub struct MinerEthereumRPC {
  client: Arc<evm::Client>,
}

pub struct MinerFilterRPC {
  filter: Mutex<FilterManager>,
}

pub struct MinerDebugRPC {}

unsafe impl Sync for MinerEthereumRPC {}
unsafe impl Sync for MinerFilterRPC {}
unsafe impl Sync for MinerDebugRPC {}

impl MinerEthereumRPC {
  pub fn new(client: Arc<evm::Client>) -> Self {
    MinerEthereumRPC { client }
  }
}

impl MinerFilterRPC {
  pub fn new(client: Arc<evm::Client>) -> Self {
    MinerFilterRPC {
      filter: Mutex::new(FilterManager::new(client)),
    }
  }
}

impl MinerDebugRPC {
  pub fn new() -> Self {
    MinerDebugRPC {}
  }
}

impl EthereumRPC for MinerEthereumRPC {
  fn client_version(&self) -> Result<String, Error> {
    info!("client_version");
    Ok("sputnikvm-dev/v0.1".to_string())
  }

  fn sha3(&self, data: Bytes) -> Result<Hex<H256>, Error> {
    info!("sha3");
    use sha3::{Digest, Keccak256};
    Ok(Hex(H256::from(Keccak256::digest(&data.0).as_slice())))
  }

  fn network_id(&self) -> Result<String, Error> {
    info!("network_id");
    Ok(format!("{}", 4447))
  }

  fn is_listening(&self) -> Result<bool, Error> {
    info!("is_listening");
    Ok(false)
  }

  fn peer_count(&self) -> Result<Hex<usize>, Error> {
    info!("peer_count");
    Ok(Hex(0))
  }

  fn protocol_version(&self) -> Result<String, Error> {
    info!("protocol_version");
    Ok(format!("{}", 63))
  }

  fn is_syncing(&self) -> Result<bool, Error> {
    info!("is_syncing");
    Ok(false)
  }

  fn coinbase(&self) -> Result<Hex<Address>, Error> {
    info!("coinbase");
    Ok(Hex(Address::default()))
  }

  fn is_mining(&self) -> Result<bool, Error> {
    info!("is_mining");
    Ok(true)
  }

  fn hashrate(&self) -> Result<String, Error> {
    info!("hashrate");
    Ok(format!("{}", 0))
  }

  fn gas_price(&self) -> Result<Hex<U256>, Error> {
    info!("gas_price");
    Ok(Hex(U256::zero()))
  }

  fn accounts(&self) -> Result<Vec<Hex<Address>>, Error> {
    info!("accounts");
    Ok(Vec::new())
  }

  fn compilers(&self) -> Result<Vec<String>, Error> {
    info!("compilers");
    Ok(Vec::new())
  }

  fn block_number(&self) -> Result<Hex<usize>, Error> {
    info!("block_number");
    let block_height = self.client.get_block_height(false).wait().unwrap();
    Ok(Hex(block_height.as_usize()))
  }

  fn balance(&self, address: Hex<Address>, block: Trailing<String>) -> Result<Hex<U256>, Error> {
    info!("balance: address = {:?}", address);

    let response = self.client.get_account_balance(address.0).wait().unwrap();
    info!("Response: {:?}", response);

    Ok(Hex(response))
  }

  fn storage_at(
    &self,
    address: Hex<Address>,
    key: Hex<H256>,
    block: Trailing<String>,
  ) -> Result<Hex<H256>, Error> {
    info!("storage_at: address = {:?}, index = {:?}", address, key);

    let response = self
      .client
      .get_storage_at((address.0, key.0))
      .wait()
      .unwrap();
    info!("Response: {:?}", response);

    Ok(Hex(response))
  }

  fn transaction_count(
    &self,
    address: Hex<Address>,
    block: Trailing<String>,
  ) -> Result<Hex<U256>, Error> {
    info!("transaction_count: address = {:?}", address);

    let response = self.client.get_account_nonce(address.0).wait().unwrap();
    info!("Response: {:?}", response);

    Ok(Hex(response))
  }

  fn block_transaction_count_by_hash(&self, block: Hex<H256>) -> Result<Option<Hex<usize>>, Error> {
    info!("block_transaction_count_by_hash: block = {:?}", block);
    /*
        println!("\n*** block_transaction_count_by_hash");

        let state = self.state.lock().unwrap();

        let block = match state.get_block_by_hash(block.0) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        Ok(Some(Hex(block.transactions.len())))
        */
    Err(Error::TODO)
  }

  fn block_transaction_count_by_number(&self, number: String) -> Result<Option<Hex<usize>>, Error> {
    info!("block_transaction_count_by_number: number = {:?}", number);
    /*
        println!("\n*** block_transaction_count_by_number *** number = {:?}", number);

        let state = self.state.lock().unwrap();

        let number = match from_block_number(&state, number) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let block = state.get_block_by_number(number);

        Ok(Some(Hex(block.transactions.len())))
        */
    Err(Error::TODO)
  }

  fn block_uncles_count_by_hash(&self, block: Hex<H256>) -> Result<Option<Hex<usize>>, Error> {
    info!("block_uncles_count_by_hash: block = {:?}", block);
    /*
        println!("\n*** block_uncles_count_by_hash");
        let state = self.state.lock().unwrap();

        let block = match state.get_block_by_hash(block.0) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        Ok(Some(Hex(block.ommers.len())))
        */
    Err(Error::TODO)
  }

  fn block_uncles_count_by_number(&self, number: String) -> Result<Option<Hex<usize>>, Error> {
    info!("block_uncles_count_by_number: number = {:?}", number);
    /*
        println!("\n*** block_uncles_count_by_number *** number = {:?}", number);
        let state = self.state.lock().unwrap();

        let number = match from_block_number(&state, number) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let block = state.get_block_by_number(number);

        Ok(Some(Hex(block.ommers.len())))
        */
    Err(Error::TODO)
  }

  fn code(&self, address: Hex<Address>, block: Trailing<String>) -> Result<Bytes, Error> {
    // currently supports only "latest" block semantics
    info!("code: address = {:?}", address);

    let response = self.client.get_account_code(address.0).wait().unwrap();
    info!("Response: {:?}", response);

    Ok(Bytes(hex::decode(&response)?))
  }

  fn sign(&self, address: Hex<Address>, message: Bytes) -> Result<Bytes, Error> {
    // this will not be implemented, as we will never store private keys
    Err(Error::NotImplemented)
  }

  fn send_transaction(&self, transaction: RPCTransaction) -> Result<Hex<H256>, Error> {
    info!("send_transaction: transaction = {:?}", transaction);

    let mut _transaction = to_evm_transaction(transaction).unwrap();
    let response = self
      .client
      .debug_execute_unsigned_transaction(_transaction)
      .wait()
      .unwrap();
    info!("Response: {:?}", response);

    Ok(Hex(response))
  }

  fn send_raw_transaction(&self, data: Bytes) -> Result<Hex<H256>, Error> {
    info!("send_raw_transaction: data = {:?}", data);

    let response = match self
      .client
      .execute_raw_transaction(hex::encode(&data.0))
      .wait()
    {
      Ok(val) => val,
      Err(_) => return Err(Error::CallError),
    };
    info!("Response: {:?}", response);

    Ok(Hex(response))
  }

  fn call(&self, transaction: RPCTransaction, block: Trailing<String>) -> Result<Bytes, Error> {
    info!("call: transaction = {:?}", transaction);

    let mut _transaction = to_evm_transaction(transaction).unwrap();
    let response = self
      .client
      .simulate_transaction(_transaction)
      .wait()
      .unwrap();
    info!("Response: {:?}", response);

    Ok(Bytes(hex::decode(&response.result)?))
  }

  fn estimate_gas(
    &self,
    transaction: RPCTransaction,
    block: Trailing<String>,
  ) -> Result<Hex<U256>, Error> {
    info!("estimate_gas: transaction = {:?}", transaction);

    // just simulate the transaction and return used_gas
    let mut _transaction = to_evm_transaction(transaction).unwrap();
    let response = self
      .client
      .simulate_transaction(_transaction)
      .wait()
      .unwrap();
    info!("Response: {:?}", response);

    Ok(Hex(response.used_gas))
  }

  fn block_by_hash(&self, hash: Hex<H256>, full: bool) -> Result<Option<RPCBlock>, Error> {
    info!("block_by_hash: hash = {:?}, full = {:?}", hash, full);
    /*
        println!("\n*** block_by_hash *** hash = {:?}", hash);
        let state = self.state.lock().unwrap();
        let block = match state.get_block_by_hash(hash.0) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let total = match state.get_total_header_by_hash(hash.0) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(to_rpc_block(block, total, full)))
        */
    Err(Error::TODO)
  }

  fn block_by_number(&self, number: String, full: bool) -> Result<Option<RPCBlock>, Error> {
    info!("block_by_number: number = {:?}, full = {:?}", number, full);

    let request = BlockRequest {
      number: number,
      full: full,
    };

    let response = match self.client.get_block_by_number(request).wait() {
      Ok(val) => val,
      // FIXME: We want to differentiate between input formatting vs network errors.
      // We have an Ekiden Error, which currently gives us only a string description.
      // We will handle invalid input as a special case for now. Improving Ekiden Errors
      // is tracked in https://github.com/oasislabs/ekiden/issues/161
      Err(e) => {
        if e.message == INVALID_BLOCK_NUMBER {
          return Err(Error::InvalidParams);
        } else {
          panic!("Contract call failed");
        }
      }
    };
    info!("Response: {:?}", response);

    match response {
      Some(block) => Ok(Some(to_rpc_block(block, full)?)),
      None => Ok(None),
    }
  }

  fn transaction_by_hash(&self, hash: Hex<H256>) -> Result<Option<RPCTransaction>, Error> {
    info!("transaction_by_hash: hash = {:?}", hash);

    let response = self.client.get_transaction_record(hash.0).wait().unwrap();
    info!("Response: {:?}", response);

    Ok(match response {
      Some(record) => Some(to_rpc_transaction(&record)?),
      None => None,
    })
  }

  fn transaction_by_block_hash_and_index(
    &self,
    block_hash: Hex<H256>,
    index: Hex<U256>,
  ) -> Result<Option<RPCTransaction>, Error> {
    info!(
      "transaction_by_block_hash_and_index: block_hash = {:?}, index = {:?}",
      block_hash, index
    );
    /*
        println!("\n*** transaction_by_block_hash_and_index *** hash = {:?}, index = {:?}", block_hash, index);

        let state = self.state.lock().unwrap();

        let block = match state.get_block_by_hash(block_hash.0) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if index.0.as_usize() >= block.transactions.len() {
            return Ok(None);
        }
        let transaction = block.transactions[index.0.as_usize()].clone();

        Ok(Some(to_rpc_transaction(transaction, Some(&block))))
        */
    Err(Error::TODO)
  }

  fn transaction_by_block_number_and_index(
    &self,
    number: String,
    index: Hex<U256>,
  ) -> Result<Option<RPCTransaction>, Error> {
    info!(
      "transaction_by_block_number_and_index: number = {:?}, index = {:?}",
      number, index
    );
    /*
        println!("\n*** transaction_by_block_number_and_index *** number = {:?}, index = {:?}", number, index);

        let state = self.state.lock().unwrap();

        let number = match from_block_number(&state, Some(number)) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let block = state.get_block_by_number(number);
        if index.0.as_usize() >= block.transactions.len() {
            return Ok(None);
        }
        let transaction = block.transactions[index.0.as_usize()].clone();

        Ok(Some(to_rpc_transaction(transaction, Some(&block))))
        */
    Err(Error::TODO)
  }

  fn transaction_receipt(&self, hash: Hex<H256>) -> Result<Option<RPCReceipt>, Error> {
    info!("transaction_receipt: hash = {:?}", hash);

    let response = self.client.get_transaction_record(hash.0).wait().unwrap();
    info!("Response: {:?}", response);

    Ok(match response {
      Some(record) => Some(to_rpc_receipt(&record)?),
      None => None,
    })
  }

  fn uncle_by_block_hash_and_index(
    &self,
    block_hash: Hex<H256>,
    index: Hex<U256>,
  ) -> Result<Option<RPCBlock>, Error> {
    info!(
      "uncle_by_block_hash_and_index: block_hash = {:?}, index = {:?}",
      block_hash, index
    );
    /*
        println!("\n*** uncle_by_block_hash_and_index *** block_hash = {:?}, index = {:?}", block_hash, index);

        let state = self.state.lock().unwrap();

        let index = index.0.as_usize();
        let block_hash = block_hash.0;
        let block = match state.get_block_by_hash(block_hash) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let uncle_hash = block.ommers[index].header_hash();
        let uncle = match state.get_block_by_hash(uncle_hash) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let total = match state.get_total_header_by_hash(uncle_hash) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        Ok(Some(to_rpc_block(uncle, total, false)))
        */
    Err(Error::TODO)
  }

  fn uncle_by_block_number_and_index(
    &self,
    block_number: String,
    index: Hex<U256>,
  ) -> Result<Option<RPCBlock>, Error> {
    info!(
      "uncle_by_block_number_and_index: block_number = {:?}, index = {:?}",
      block_number, index
    );
    /*
        println!("\n*** uncle_by_block_number_and_index *** block_number = {:?}, index = {:?}", block_number, index);

        let state = self.state.lock().unwrap();

        let block_number = match from_block_number(&state, Some(block_number)) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let index = index.0.as_usize();
        let block = state.get_block_by_number(block_number);
        let uncle_hash = block.ommers[index].header_hash();
        let uncle = match state.get_block_by_hash(uncle_hash) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let total = match state.get_total_header_by_hash(uncle_hash) {
            Ok(val) => val,
            Err(Error::NotFound) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        Ok(Some(to_rpc_block(uncle, total, false)))
        */
    Err(Error::TODO)
  }

  fn logs(&self, log: RPCLogFilter) -> Result<Vec<RPCLog>, Error> {
    info!("logs: log = {:?}", log);
    /*
        println!("\n*** logs. log = {:?}", log);

        let state = self.state.lock().unwrap();

        match from_log_filter(&state, log) {
            Ok(filter) => Ok(get_logs(&state, filter)?),
            Err(_) => Ok(Vec::new()),
        }
        */
    Err(Error::TODO)
  }
}

impl FilterRPC for MinerFilterRPC {
  fn new_filter(&self, log: RPCLogFilter) -> Result<String, Error> {
    info!("new_filter");
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn new_block_filter(&self) -> Result<String, Error> {
    info!("new_block_filter");
    let id = self.filter.lock().unwrap().install_block_filter();
    Ok(format!("0x{:x}", id))
  }

  fn new_pending_transaction_filter(&self) -> Result<String, Error> {
    info!("pending_transaction_filter");
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn uninstall_filter(&self, id: String) -> Result<bool, Error> {
    info!("uninstall_filter: id = {:?}", id);
    let id = U256::from_str(&id)?.as_usize();
    self.filter.lock().unwrap().uninstall_filter(id);
    Ok(true)
  }

  fn filter_changes(&self, id: String) -> Result<Either<Vec<String>, Vec<RPCLog>>, Error> {
    info!("filter_changes: id = {:?}", id);

    let id = U256::from_str(&id)?.as_usize();
    let result = self.filter.lock().unwrap().get_changes(id)?;

    info!("Response: {:?}", result);
    Ok(result)
  }

  fn filter_logs(&self, id: String) -> Result<Vec<RPCLog>, Error> {
    info!("filter_logs: id = {:?}", id);
    // FIXME: implement
    Err(Error::NotImplemented)
  }
}

impl DebugRPC for MinerDebugRPC {
  fn block_rlp(&self, number: usize) -> Result<Bytes, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn trace_transaction(
    &self,
    hash: Hex<H256>,
    config: Trailing<RPCTraceConfig>,
  ) -> Result<RPCTrace, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn trace_block(
    &self,
    block_rlp: Bytes,
    config: Trailing<RPCTraceConfig>,
  ) -> Result<RPCBlockTrace, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn trace_block_by_number(
    &self,
    number: usize,
    config: Trailing<RPCTraceConfig>,
  ) -> Result<RPCBlockTrace, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn trace_block_by_hash(
    &self,
    hash: Hex<H256>,
    config: Trailing<RPCTraceConfig>,
  ) -> Result<RPCBlockTrace, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn trace_block_from_file(
    &self,
    path: String,
    config: Trailing<RPCTraceConfig>,
  ) -> Result<RPCBlockTrace, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }

  fn dump_block(&self, number: usize) -> Result<RPCDump, Error> {
    // FIXME: implement
    Err(Error::NotImplemented)
  }
}
