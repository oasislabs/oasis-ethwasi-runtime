use super::{DebugRPC, Either, EthereumRPC, FilterRPC, RPCBlock, RPCBlockTrace, RPCDump, RPCLog,
            RPCLogFilter, RPCReceipt, RPCTrace, RPCTraceConfig, RPCTransaction};
use super::serialize::*;
use super::util::*;

use error::Error;

use bigint::{Address, Gas, H256, M256, U256};
use evm_api::{AccountRequest, BlockRequest, ExecuteRawTransactionRequest,
              ExecuteTransactionRequest, TransactionRecordRequest};
use sputnikvm::Patch;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use jsonrpc_macros::Trailing;

use ekiden_rpc_client;
use evm;
use futures::future::Future;

use hexutil::{read_hex, to_hex};

pub struct MinerEthereumRPC<P: Patch + Send> {
    client: Arc<Mutex<evm::Client<ekiden_rpc_client::backend::Web3RpcClientBackend>>>,
    _patch: PhantomData<P>,
}

pub struct MinerFilterRPC<P: Patch + Send> {
    _patch: PhantomData<P>,
}

pub struct MinerDebugRPC<P: Patch + Send> {
    _patch: PhantomData<P>,
}

unsafe impl<P: Patch + Send> Sync for MinerEthereumRPC<P> {}
unsafe impl<P: Patch + Send> Sync for MinerFilterRPC<P> {}
unsafe impl<P: Patch + Send> Sync for MinerDebugRPC<P> {}

impl<P: Patch + Send> MinerEthereumRPC<P> {
    pub fn new(
        client: Arc<Mutex<evm::Client<ekiden_rpc_client::backend::Web3RpcClientBackend>>>,
    ) -> Self {
        MinerEthereumRPC {
            client,
            _patch: PhantomData,
        }
    }
}

impl<P: Patch + Send> MinerFilterRPC<P> {
    pub fn new() -> Self {
        MinerFilterRPC {
            _patch: PhantomData,
        }
    }
}

impl<P: Patch + Send> MinerDebugRPC<P> {
    pub fn new() -> Self {
        MinerDebugRPC {
            _patch: PhantomData,
        }
    }
}

impl<P: 'static + Patch + Send> EthereumRPC for MinerEthereumRPC<P> {
    fn client_version(&self) -> Result<String, Error> {
        println!("\n*** client_version");
        Ok("sputnikvm-dev/v0.1".to_string())
    }

    fn sha3(&self, data: Bytes) -> Result<Hex<H256>, Error> {
        println!("\n*** sha3");
        use sha3::{Digest, Keccak256};
        Ok(Hex(H256::from(Keccak256::digest(&data.0).as_slice())))
    }

    fn network_id(&self) -> Result<String, Error> {
        // println!("\n*** network_id. Result: 4447");
        Ok(format!("{}", 4447))
    }

    fn is_listening(&self) -> Result<bool, Error> {
        println!("\n*** is_listening");
        Ok(false)
    }

    fn peer_count(&self) -> Result<Hex<usize>, Error> {
        println!("\n*** peer_count");
        Ok(Hex(0))
    }

    fn protocol_version(&self) -> Result<String, Error> {
        println!("\n*** protocol_version");
        Ok(format!("{}", 63))
    }

    fn is_syncing(&self) -> Result<bool, Error> {
        println!("\n*** is_syncing");
        Ok(false)
    }

    fn coinbase(&self) -> Result<Hex<Address>, Error> {
        println!("\n*** coinbase");
        Ok(Hex(Address::default()))
    }

    fn is_mining(&self) -> Result<bool, Error> {
        println!("\n*** is_mining");
        Ok(true)
    }

    fn hashrate(&self) -> Result<String, Error> {
        println!("\n*** hashrate");
        Ok(format!("{}", 0))
    }

    fn gas_price(&self) -> Result<Hex<Gas>, Error> {
        println!("\n*** gas_price");
        Ok(Hex(Gas::zero()))
    }

    fn accounts(&self) -> Result<Vec<Hex<Address>>, Error> {
        /*
        let state = self.state.lock().unwrap();

         println!("\n*** Accounts");

        let result = Ok(state.accounts().iter().map(|key| {
            println!("    Key: {:?}", key);
            Address::from_secret_key(key).unwrap()
        }).map(|address| {
            println!("    Address: {:?}", Hex(address));
            Hex(address)
        }).collect());

        //println!("Result: {:?}", result);
        result
        */
        Err(Error::TODO)
    }

    fn block_number(&self) -> Result<Hex<usize>, Error> {
        /*
        println!("\n*** block_number");
        let state = self.state.lock().unwrap();

        Ok(Hex(state.block_height()))
        */
        Err(Error::TODO)
    }

    fn balance(&self, address: Hex<Address>, block: Trailing<String>) -> Result<Hex<U256>, Error> {
        println!("\n*** balance *** address = {:?}", address);

        let mut client = self.client.lock().unwrap();

        let mut request = AccountRequest::new();
        request.set_address(address.0.hex());

        let response = client.get_account_balance(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Hex(U256::from_dec_str(response.get_balance()).unwrap()))
    }

    fn storage_at(
        &self,
        address: Hex<Address>,
        index: Hex<U256>,
        block: Trailing<String>,
    ) -> Result<Hex<M256>, Error> {
        /*
        println!("\n*** storage_at *** address = {:?}, index = {:?}", address, index);

        let state = self.state.lock().unwrap();

        let block = from_block_number(&state, block)?;

        let block = state.get_block_by_number(block);
        let stateful = state.stateful();
        let trie = stateful.state_of(block.header.state_root);

        let account: Option<Account> = trie.get(&address.0);
        match account {
            Some(account) => {
                let storage = stateful.storage_state_of(account.storage_root);
                let value = storage.get(&H256::from(index.0)).unwrap_or(M256::zero());
                Ok(Hex(value))
            },
            None => {
                Ok(Hex(M256::zero()))
            },
        }*/
        Err(Error::TODO)
    }

    fn transaction_count(
        &self,
        address: Hex<Address>,
        block: Trailing<String>,
    ) -> Result<Hex<U256>, Error> {
        println!("\n*** transaction_count *** address = {:?}", address);

        let mut client = self.client.lock().unwrap();

        let mut request = AccountRequest::new();
        request.set_address(address.0.hex());

        let response = client.get_account_nonce(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Hex(U256::from_dec_str(response.get_nonce()).unwrap()))
    }

    fn block_transaction_count_by_hash(
        &self,
        block: Hex<H256>,
    ) -> Result<Option<Hex<usize>>, Error> {
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

    fn block_transaction_count_by_number(
        &self,
        number: String,
    ) -> Result<Option<Hex<usize>>, Error> {
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
        println!("\n*** code *** address = {:?}", address);

        let mut client = self.client.lock().unwrap();

        let mut request = AccountRequest::new();
        request.set_address(address.0.hex());

        let response = client.get_account_code(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Bytes(read_hex(response.get_code())?))
    }

    fn sign(&self, address: Hex<Address>, message: Bytes) -> Result<Bytes, Error> {
        /*
        println!("\n*** sign *** address = {:?}, message = {:?}", address, message);

        use sha3::{Digest, Keccak256};
        use secp256k1::{SECP256K1, Message};

        let state = self.state.lock().unwrap();

        let mut signing_message = Vec::new();

        signing_message.extend("Ethereum Signed Message:\n".as_bytes().iter().cloned());
        signing_message.extend(format!("0x{:x}\n", message.0.len()).as_bytes().iter().cloned());
        signing_message.extend(message.0.iter().cloned());

        let hash = H256::from(Keccak256::digest(&signing_message).as_slice());
        let secret_key = {
            let mut secret_key = None;
            for key in state.accounts() {
                if Address::from_secret_key(&key)? == address.0 {
                    secret_key = Some(key);
                }
            }
            match secret_key {
                Some(val) => val,
                None => return Err(Error::NotFound),
            }
        };
        let sign = SECP256K1.sign_recoverable(&Message::from_slice(&hash).unwrap(), &secret_key)?;
        let (rec, sign) = sign.serialize_compact(&SECP256K1);
        let mut ret = Vec::new();
        ret.push(rec.to_i32() as u8);
        ret.extend(sign.as_ref());

        Ok(Bytes(ret))
        */
        Err(Error::TODO)
    }

    fn send_transaction(&self, mut transaction: RPCTransaction) -> Result<Hex<H256>, Error> {
        println!("\n*** send_transaction");

        let mut _transaction = to_evm_transaction(transaction).unwrap();

        let mut client = self.client.lock().unwrap();

        let mut request = ExecuteTransactionRequest::new();
        request.set_transaction(_transaction);

        let response = client
            .debug_execute_unsigned_transaction(request)
            .wait()
            .unwrap();
        println!("    Response: {:?}", response);

        Ok(Hex(H256::from_str(response.get_hash()).unwrap()))
    }

    fn send_raw_transaction(&self, data: Bytes) -> Result<Hex<H256>, Error> {
        println!("\n*** send_raw_transaction *** data = {:?}", data);

        let mut client = self.client.lock().unwrap();

        let mut request = ExecuteRawTransactionRequest::new();
        request.set_data(to_hex(&data.0));

        let response = match client.execute_raw_transaction(request).wait() {
            Ok(val) => val,
            Err(_) => return Err(Error::CallError),
        };

        Ok(Hex(H256::from_str(response.get_hash()).unwrap()))
    }

    fn call(&self, transaction: RPCTransaction, block: Trailing<String>) -> Result<Bytes, Error> {
        println!("\n*** Call contract");
        let mut _transaction = to_evm_transaction(transaction).unwrap();

        let mut client = self.client.lock().unwrap();

        let mut request = ExecuteTransactionRequest::new();
        request.set_transaction(_transaction);

        println!("*** Call transaction");
        println!("Transaction: {:?}", request.get_transaction());

        let response = client.simulate_transaction(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Bytes(response.get_result().as_bytes().to_vec()))
    }

    fn estimate_gas(
        &self,
        transaction: RPCTransaction,
        block: Trailing<String>,
    ) -> Result<Hex<Gas>, Error> {
        println!("\n*** estimate_gas");
        let mut _transaction = to_evm_transaction(transaction).unwrap();

        let mut client = self.client.lock().unwrap();

        // just simulate the transaction and return used_gas
        let mut request = ExecuteTransactionRequest::new();
        request.set_transaction(_transaction);

        let response = client.simulate_transaction(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Hex(Gas::from_str(response.get_used_gas()).unwrap()))
    }

    fn block_by_hash(&self, hash: Hex<H256>, full: bool) -> Result<Option<RPCBlock>, Error> {
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
        println!("\n*** block_by_number");

        let mut client = self.client.lock().unwrap();

        let mut request = BlockRequest::new();
        request.set_number(number);
        request.set_full(full);

        let response = client.get_block_by_number(request).wait().unwrap();
        println!("    Response: {:?}", response);

        if response.has_block() {
            Ok(Some(to_rpc_block(response.get_block(), full)?))
        } else {
            // return null if block doesn't exist
            Ok(None)
        }
    }

    fn transaction_by_hash(&self, hash: Hex<H256>) -> Result<Option<RPCTransaction>, Error> {
        println!("\n*** transaction_by_hash");

        let mut client = self.client.lock().unwrap();

        let mut request = TransactionRecordRequest::new();
        request.set_hash(format!("{:x}", hash.0));

        let response = client.get_transaction_record(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Some(to_rpc_transaction(response.get_record())?))
    }

    fn transaction_by_block_hash_and_index(
        &self,
        block_hash: Hex<H256>,
        index: Hex<U256>,
    ) -> Result<Option<RPCTransaction>, Error> {
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
        println!("\n*** transaction_receipt");

        let mut client = self.client.lock().unwrap();

        let mut request = TransactionRecordRequest::new();
        request.set_hash(format!("{:x}", hash.0));

        let response = client.get_transaction_record(request).wait().unwrap();
        println!("    Response: {:?}", response);

        Ok(Some(to_rpc_receipt(response.get_record())?))
    }

    fn uncle_by_block_hash_and_index(
        &self,
        block_hash: Hex<H256>,
        index: Hex<U256>,
    ) -> Result<Option<RPCBlock>, Error> {
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

    fn compilers(&self) -> Result<Vec<String>, Error> {
        println!("\n*** compilers");

        Ok(Vec::new())
    }

    fn logs(&self, log: RPCLogFilter) -> Result<Vec<RPCLog>, Error> {
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

impl<P: 'static + Patch + Send> FilterRPC for MinerFilterRPC<P> {
    fn new_filter(&self, log: RPCLogFilter) -> Result<String, Error> {
        // FIXME: implement
        Err(Error::NotImplemented)
    }

    fn new_block_filter(&self) -> Result<String, Error> {
        // FIXME: implement
        Err(Error::NotImplemented)
    }

    fn new_pending_transaction_filter(&self) -> Result<String, Error> {
        // FIXME: implement
        Err(Error::NotImplemented)
    }

    fn uninstall_filter(&self, id: String) -> Result<bool, Error> {
        // FIXME: implement
        Err(Error::NotImplemented)
    }

    fn filter_changes(&self, id: String) -> Result<Either<Vec<String>, Vec<RPCLog>>, Error> {
        // FIXME: implement
        Err(Error::NotImplemented)
    }

    fn filter_logs(&self, id: String) -> Result<Vec<RPCLog>, Error> {
        // FIXME: implement
        Err(Error::NotImplemented)
    }
}

impl<P: 'static + Patch + Send> DebugRPC for MinerDebugRPC<P> {
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
