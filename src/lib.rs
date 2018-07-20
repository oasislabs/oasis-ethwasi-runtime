#![feature(iterator_try_fold)]
#![feature(use_extern_macros)]

extern crate common_types as ethcore_types;
extern crate ekiden_core;
extern crate ekiden_trusted;
extern crate ethcore;
extern crate ethereum_api;
extern crate ethereum_types;
extern crate hex;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate protobuf;
extern crate sha3;

mod evm;
#[macro_use]
mod logger;
mod state;
mod storage;

use ekiden_core::error::{Error, Result};
use ekiden_trusted::{contract::create_contract, enclave::enclave_init};
use ethcore::{rlp,
              transaction::{Action, SignedTransaction, Transaction as EthcoreTransaction,
                            UnverifiedTransaction}};
use ethereum_api::{with_api, AccountState, BlockId, ExecuteTransactionResponse, Filter, Log,
                   Receipt, SimulateTransactionResponse, Transaction, TransactionRequest};
use ethereum_types::{Address, H256, U256};

use state::{add_block, block_by_hash, block_by_number, block_hash, get_latest_block_number,
            new_block};

enclave_init!();

// Create enclave contract interface.
with_api! {
    create_contract!(api);
}

// used for performance debugging
fn debug_null_call(_request: &bool) -> Result<()> {
    Ok(())
}

fn strip_0x<'a>(hex: &'a str) -> &'a str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

fn from_hex<S: AsRef<str>>(hex: S) -> Result<Vec<u8>> {
    Ok(hex::decode(strip_0x(hex.as_ref()))?)
}

#[cfg(any(debug_assertions, feature = "benchmark"))]
fn inject_accounts(accounts: &Vec<AccountState>) -> Result<()> {
    let mut block = new_block()?;
    accounts.iter().try_for_each(|ref account| {
        block.block_mut().state_mut().new_contract(
            &account.address,
            account.balance.clone(),
            account.nonce.clone(),
        );
        if account.code.len() > 0 {
            block
                .block_mut()
                .state_mut()
                .init_code(&account.address, from_hex(&account.code)?)
                .map_err(|_| {
                    Error::new(format!(
                        "Could not init code for address {:?}.",
                        &account.address
                    ))
                })
        } else {
            Ok(())
        }
    })?;

    // commit state changes
    block.block_mut().state_mut().commit()?;

    // set timestamp to 0, as blocks must be deterministic
    block.set_timestamp(0);

    add_block(block.close_and_lock())?;
    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn inject_accounts(accounts: &Vec<AccountState>) -> Result<()> {
    Err(Error::new(
        "API available only in debug and benchmarking builds",
    ))
}

#[cfg(any(debug_assertions, feature = "benchmark"))]
pub fn inject_account_storage(storages: &Vec<(Address, H256, H256)>) -> Result<()> {
    let mut block = new_block()?;
    storages.iter().try_for_each(|&(addr, key, value)| {
        block
            .block_mut()
            .state_mut()
            .set_storage(&addr, key.clone(), value.clone())
            .map_err(|_| Error::new("Could not set storage."))
    })?;

    // commit state changes
    block.block_mut().state_mut().commit()?;

    // set timestamp to 0, as blocks must be deterministic
    block.set_timestamp(0);

    add_block(block.close_and_lock())?;
    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "benchmark")))]
fn inject_account_storage(storage: &Vec<(Address, H256, H256)>) -> Result<()> {
    Err(Error::new(
        "API available only in debug and benchmarking builds",
    ))
}

/// TODO: first argument is ignored; remove once APIs support zero-argument signatures (#246)
pub fn get_block_height(_request: &bool) -> Result<U256> {
    Ok(get_latest_block_number().into())
}

fn get_block_hash(id: &BlockId) -> Result<Option<H256>> {
    let hash = match *id {
        BlockId::Hash(hash) => Some(hash),
        BlockId::Number(number) => block_hash(number.into()),
        BlockId::Earliest => block_hash(0),
        BlockId::Latest => block_hash(get_latest_block_number()),
    };
    Ok(hash)
}

fn get_block(id: &BlockId) -> Result<Option<Vec<u8>>> {
    debug!("get_block, id: {:?}", id);

    let block = match *id {
        BlockId::Hash(hash) => block_by_hash(hash),
        BlockId::Number(number) => block_by_number(number.into()),
        BlockId::Earliest => block_by_number(0),
        BlockId::Latest => block_by_number(get_latest_block_number()),
    };

    match block {
        Some(block) => Ok(Some(block.into_inner())),
        None => Ok(None),
    }
}

fn get_logs(filter: &Filter) -> Result<Vec<Log>> {
    debug!("get_logs, filter: {:?}", filter);
    Ok(state::get_logs(filter))
}

pub fn get_transaction(hash: &H256) -> Result<Option<Transaction>> {
    debug!("get_transaction, hash: {:?}", hash);
    Ok(state::get_transaction(hash))
}

pub fn get_receipt(hash: &H256) -> Result<Option<Receipt>> {
    debug!("get_receipt, hash: {:?}", hash);
    Ok(state::get_receipt(hash))
}

pub fn get_account_balance(address: &Address) -> Result<U256> {
    debug!("get_account_balance, address: {:?}", address);
    state::get_account_balance(address)
}

pub fn get_account_nonce(address: &Address) -> Result<U256> {
    debug!("get_account_nonce, address: {:?}", address);
    state::get_account_nonce(address)
}

pub fn get_account_code(address: &Address) -> Result<Option<Vec<u8>>> {
    debug!("get_account_code, address: {:?}", address);
    state::get_account_code(address)
}

pub fn get_storage_at(pair: &(Address, H256)) -> Result<H256> {
    debug!("get_storage_at, address: {:?}", pair);
    state::get_account_storage(pair.0, pair.1)
}

pub fn execute_raw_transaction(request: &Vec<u8>) -> Result<ExecuteTransactionResponse> {
    debug!("execute_raw_transaction");
    let decoded: UnverifiedTransaction = match rlp::decode(request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
            })
        }
    };
    let is_create = decoded.as_unsigned().action == Action::Create;
    let signed = match SignedTransaction::new(decoded) {
        Ok(t) => t,
        Err(e) => {
            return Ok(ExecuteTransactionResponse {
                hash: Err(e.to_string()),
                created_contract: false,
            })
        }
    };
    let result = transact(signed).map_err(|e| e.to_string());
    Ok(ExecuteTransactionResponse {
        created_contract: if result.is_err() { false } else { is_create },
        hash: result,
    })
}

fn transact(transaction: SignedTransaction) -> Result<H256> {
    let mut block = new_block()?;
    let tx_hash = transaction.hash();
    block.push_transaction(transaction, None)?;
    // set timestamp to 0, as blocks must be deterministic
    block.set_timestamp(0);
    add_block(block.close_and_lock())?;
    Ok(tx_hash)
}

fn make_unsigned_transaction(request: &TransactionRequest) -> Result<SignedTransaction> {
    let tx = EthcoreTransaction {
        action: if request.is_call {
            Action::Call(request
                .address
                .ok_or(Error::new("Must provide address for call transaction."))?)
        } else {
            Action::Create
        },
        value: request.value.unwrap_or(U256::zero()),
        data: request.input.clone().unwrap_or(vec![]),
        gas: U256::max_value(),
        gas_price: U256::zero(),
        nonce: request.nonce.unwrap_or_else(|| {
            request
                .caller
                .map(|addr| state::get_account_nonce(&addr).unwrap_or(U256::zero()))
                .unwrap_or(U256::zero())
        }),
    };
    Ok(match request.caller {
        Some(addr) => tx.fake_sign(addr),
        None => tx.null_sign(0),
    })
}

pub fn simulate_transaction(request: &TransactionRequest) -> Result<SimulateTransactionResponse> {
    debug!("simulate_transaction");
    let tx = match make_unsigned_transaction(request) {
        Ok(t) => t,
        Err(e) => {
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                result: Err(e.to_string()),
            })
        }
    };
    let exec = match evm::simulate_transaction(&tx) {
        Ok(exec) => exec,
        Err(e) => {
            return Ok(SimulateTransactionResponse {
                used_gas: U256::from(0),
                result: Err(e.to_string()),
            })
        }
    };
    Ok(SimulateTransactionResponse {
        used_gas: exec.gas_used,
        result: Ok(exec.output),
    })
}

#[cfg(test)]
mod tests {
    use ethcore::blockchain::BlockChain;
    extern crate ethkey;

    use std::str::FromStr;
    use std::sync::Arc;
    use std::sync::Mutex;

    use self::ethkey::{KeyPair, Secret};
    use super::*;
    use ethcore::{self, vm};
    use hex;

    struct Client {
        keypair: KeyPair,
    }

    impl Client {
        fn new() -> Self {
            Self {
                // address: 0x7110316b618d20d0c44728ac2a3d683536ea682
                keypair: KeyPair::from_secret(
                    Secret::from_str(
                        "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c",
                    ).unwrap(),
                ).unwrap(),
            }
        }

        fn create_contract(&mut self, code: Vec<u8>, balance: &U256) -> (H256, Address) {
            let tx = EthcoreTransaction {
                action: Action::Create,
                nonce: get_account_nonce(&self.keypair.address()).unwrap(),
                gas_price: U256::from(0),
                gas: U256::from(1000000),
                value: *balance,
                data: code,
            }.sign(&self.keypair.secret(), None);

            let raw = rlp::encode(&tx);
            let hash = execute_raw_transaction(&raw.into_vec())
                .unwrap()
                .hash
                .unwrap();
            let receipt = get_receipt(&hash).unwrap().unwrap();
            (hash, receipt.contract_address.unwrap())
        }

        fn call(&mut self, contract: &Address, data: Vec<u8>, value: &U256) -> Vec<u8> {
            let tx = TransactionRequest {
                caller: Some(self.keypair.address()),
                is_call: true,
                address: Some(*contract),
                input: Some(data),
                value: Some(*value),
                nonce: None,
            };

            simulate_transaction(&tx).unwrap().result.unwrap()
        }
    }

    lazy_static! {
        static ref CLIENT: Mutex<Client> = Mutex::new(Client::new());
    }

    #[test]
    fn test_create_balance() {
        let mut client = CLIENT.lock().unwrap();

        let init_bal = get_account_balance(&client.keypair.address()).unwrap();
        let contract_bal = U256::from(10);
        let remaining_bal = init_bal - contract_bal;

        let init_nonce = get_account_nonce(&client.keypair.address()).unwrap();

        let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
        let (_, contract) = client.create_contract(code, &contract_bal);

        assert_eq!(
            get_account_balance(&client.keypair.address()).unwrap(),
            remaining_bal
        );
        assert_eq!(
            get_account_nonce(&client.keypair.address()).unwrap(),
            init_nonce + U256::one()
        );
        assert_eq!(get_account_balance(&contract).unwrap(), contract_bal);
        assert_eq!(
            get_storage_at(&(contract, H256::zero())).unwrap(),
            H256::from(&remaining_bal)
        );
    }

    #[test]
    fn test_solidity_blockhash() {
        // pragma solidity ^0.4.18;
        // contract The {
        //   function hash(uint64 num) public view returns (bytes32) {
        //     return blockhash(num);
        //   }
        // }

        use std::mem::transmute;

        let mut client = CLIENT.lock().unwrap();
        let blockhash_code = hex::decode("608060405234801561001057600080fd5b5060d58061001f6000396000f300608060405260043610603f576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063e432a10e146044575b600080fd5b348015604f57600080fd5b506076600480360381019080803567ffffffffffffffff1690602001909291905050506094565b60405180826000191660001916815260200191505060405180910390f35b60008167ffffffffffffffff164090509190505600a165627a7a7230582078c16bf994a1597df9b750bb680f3fc4b4e8c9c8f51607bbfcc28d9496a211d70029").unwrap();

        let (_, contract) = client.create_contract(blockhash_code, &U256::zero());

        let mut blockhash = |num: u64| -> Vec<u8> {
            let mut data = hex::decode(
                "e432a10e0000000000000000000000000000000000000000000000000000000000000000",
            ).unwrap();
            let bytes: [u8; 8] = unsafe { transmute(num.to_be()) };
            for i in 0..8 {
                data[28 + i] = bytes[i];
            }
            client.call(&contract, data, &U256::zero())
        };

        assert_eq!(
            blockhash(get_latest_block_number()),
            block_hash(get_latest_block_number()).unwrap().to_vec()
        );
    }

    #[test]
    fn test_solidity_x_contract_call() {
        // contract A {
        //   function call_a(address b, int a) public pure returns (int) {
        //       B cb = B(b);
        //       return cb.call_b(a);
        //     }
        // }
        //
        // contract B {
        //     function call_b(int b) public pure returns (int) {
        //             return b + 1;
        //         }
        // }

        let mut client = CLIENT.lock().unwrap();

        let contract_a_code = hex::decode("608060405234801561001057600080fd5b5061015d806100206000396000f3006080604052600436106100405763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663e3f300558114610045575b600080fd5b34801561005157600080fd5b5061007673ffffffffffffffffffffffffffffffffffffffff60043516602435610088565b60408051918252519081900360200190f35b6000808390508073ffffffffffffffffffffffffffffffffffffffff1663346fb5c9846040518263ffffffff167c010000000000000000000000000000000000000000000000000000000002815260040180828152602001915050602060405180830381600087803b1580156100fd57600080fd5b505af1158015610111573d6000803e3d6000fd5b505050506040513d602081101561012757600080fd5b50519493505050505600a165627a7a7230582062a004e161bd855be0a78838f92bafcbb4cef5df9f9ac673c2f7d174eff863fb0029").unwrap();
        let (_, contract_a) = client.create_contract(contract_a_code, &U256::zero());

        let contract_b_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();
        let (_, contract_b) = client.create_contract(contract_b_code, &U256::zero());

        let data = hex::decode(format!(
            "e3f30055000000000000000000000000{:\
             x}0000000000000000000000000000000000000000000000000000000000000029",
            contract_b
        )).unwrap();
        let output = client.call(&contract_a, data, &U256::zero());

        // expected output is 42
        assert_eq!(
            hex::encode(output),
            "000000000000000000000000000000000000000000000000000000000000002a"
        );
    }

    #[test]
    fn test_redeploy() {
        let mut client = CLIENT.lock().unwrap();

        let contract_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();

        // deploy once
        let (hash, contract) = client.create_contract(contract_code.clone(), &U256::zero());
        let receipt = get_receipt(&hash).unwrap().unwrap();
        let status = receipt.status_code.unwrap();
        assert_eq!(status, 1 as u64);

        // deploy again
        let (hash, contract) = client.create_contract(contract_code.clone(), &U256::zero());
        let receipt = get_receipt(&hash).unwrap().unwrap();
        let status = receipt.status_code.unwrap();
        assert_eq!(status, 1 as u64);
    }

    #[test]
    fn test_signature_verification() {
        let client = CLIENT.lock().unwrap();

        let bad_sig = EthcoreTransaction {
            action: Action::Create,
            nonce: get_account_nonce(&client.keypair.address()).unwrap(),
            gas_price: U256::from(0),
            gas: U256::from(1000000),
            value: U256::from(0),
            data: vec![],
        }.fake_sign(client.keypair.address());
        let bad_result = execute_raw_transaction(&rlp::encode(&bad_sig).into_vec())
            .unwrap()
            .hash;

        let good_sig = EthcoreTransaction {
            action: Action::Create,
            nonce: get_account_nonce(&client.keypair.address()).unwrap(),
            gas_price: U256::from(0),
            gas: U256::from(1000000),
            value: U256::from(0),
            data: vec![],
        }.sign(client.keypair.secret(), None);
        let good_result = execute_raw_transaction(&rlp::encode(&good_sig).into_vec())
            .unwrap()
            .hash;

        assert!(bad_result.is_err());
        assert!(good_result.is_ok());
    }

    fn get_account_nonce_chain(chain: &BlockChain, address: &Address) -> U256 {
        let backend = state::get_backend();
        let root = chain.best_block_header().state_root().clone();
        ethcore::state::State::from_existing(
            backend,
            root,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
        ).unwrap()
            .nonce(address)
            .unwrap()
    }

    #[test]
    fn test_hiatus() {
        let mut client = CLIENT.lock().unwrap();
        let client_address = client.keypair.address();

        // Initialize the DB.
        let reference_nonce_before = get_account_nonce(&client_address).unwrap();

        // Create a chain representing node A, which is initially the leader.
        let chain_a = BlockChain::new(
            Default::default(), /* config */
            &*evm::SPEC.genesis_block(),
            Arc::new(state::StateDb::instance()),
        );
        let nonce_a_before = get_account_nonce_chain(&chain_a, &client_address);

        // The default node becomes the leader.
        // Do some transaction. Here we deploy an empty contract.
        // pragma solidity ^0.4.24;
        // contract Empty { }
        let code_empty = hex::decode("6080604052348015600f57600080fd5b50603580601d6000396000f3006080604052600080fd00a165627a7a723058209c0fbaf927d5bcdab687e32584f12a46fbcd505bcefb4fec306c065651c73a3e0029").unwrap();
        client.create_contract(code_empty, &U256::zero());

        // Save the new nonce from the default node, which is currently leader.
        let reference_nonce = get_account_nonce(&client_address).unwrap();

        // When node A is leader again, getting the nonce should give an up to date value.
        let nonce_a = get_account_nonce_chain(&chain_a, &client_address);
        assert_eq!(nonce_a, reference_nonce);
    }

    #[test]
    fn test_last_hashes() {
        use state::{best_block_header, block_hash, last_hashes};

        let mut client = CLIENT.lock().unwrap();

        // ensure that we have >256 blocks
        for i in 0..260 {
            client.create_contract(vec![], &U256::zero());
        }

        // get last_hashes from latest block
        let last_hashes = last_hashes(&best_block_header().hash());

        assert_eq!(last_hashes.len(), 256);
        assert_eq!(
            last_hashes[1],
            block_hash(get_latest_block_number() - 1).unwrap()
        );
    }
}
