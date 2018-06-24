use std::{cmp, collections::BTreeMap, sync::Arc};

use ekiden_core::error::Result;
use ethcore::{executive::{contract_address, Executed, Executive, TransactOptions},
              machine::EthereumMachine,
              spec::CommonParams,
              transaction::{SignedTransaction, Transaction},
              vm};
use ethereum_types::{Address, H256, U256};

use super::state::{get_block_hash, get_latest_block_number, get_state, with_state};

/// as per https://github.com/paritytech/parity/blob/master/ethcore/res/ethereum/byzantium_test.json
macro_rules! evm_params {
    () => {{
        let mut params = CommonParams::default();
        params.maximum_extra_data_size = 0x20;
        params.min_gas_limit = 0x1388.into();
        params.network_id = 0x01;
        params.max_code_size = 24576;
        params.eip98_transition = <u64>::max_value();
        params.gas_limit_bound_divisor = 0x0400.into();
        params.registrar = "0xc6d9d2cd449a754c494264e1809c50e34d64562b".into();
        params
    }};
}

fn get_env_info() -> vm::EnvInfo {
    let block_number = <u64>::from(get_latest_block_number());
    let last_hashes = (0..cmp::min(block_number, 256) + 1)
        .map(|i| get_block_hash(U256::from(block_number - i)).expect("blockhash should exist?"))
        .collect();
    let mut env_info = vm::EnvInfo::default();
    env_info.last_hashes = Arc::new(last_hashes);
    env_info.number = block_number + 1;
    env_info.gas_limit = U256::max_value();
    env_info
}

pub fn execute_transaction(transaction: &SignedTransaction) -> Result<(Executed, H256)> {
    let machine = EthereumMachine::regular(evm_params!(), BTreeMap::new() /* builtins */);

    let mut transact_options = TransactOptions::with_no_tracing();
    if cfg!(feature = "benchmark") {
        // don't check nonce in benchmarking mode (transactions may be executed out of order)
        transact_options = transact_options.dont_check_nonce();
    }

    with_state(
        |state| {
            Ok(Executive::new(state, &get_env_info(), &machine)
                .transact(&transaction, transact_options)?)
        },
    )
}

pub fn simulate_transaction(transaction: &SignedTransaction) -> Result<(Executed, H256)> {
    let machine = EthereumMachine::regular(evm_params!(), BTreeMap::new() /* builtins */);

    let mut state = get_state()?;
    let exec = Executive::new(&mut state, &get_env_info(), &machine)
        .transact_virtual(&transaction, TransactOptions::with_no_tracing())?;
    let (root, _db) = state.drop();
    Ok((exec, root))
}

pub fn get_contract_address(transaction: &Transaction) -> Address {
    contract_address(
        vm::CreateContractAddress::FromCodeHash,
        &Address::zero(), // unused
        &U256::zero(),    // unused
        &transaction.data,
    ).0
}

#[cfg(test)]
mod tests {
    use super::{super::{miner, state, util::strip_0x},
                *};

    use std::default::Default;

    use ethcore::{self,
                  executive::contract_address,
                  journaldb::overlaydb::OverlayDB,
                  state::backend::Basic as BasicBackend,
                  transaction::{Action, Transaction}};
    use ethereum_types::Address;
    use hex;

    fn new_state() -> Result<ethcore::state::State<BasicBackend<OverlayDB>>> {
        Ok(ethcore::state::State::new(
            state::get_backend(),
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
        ))
    }

    struct Client {
        address: Address,
        nonce: U256,
    }

    impl Client {
        fn new(balance: &U256) -> Self {
            let sender = Self {
                address: Address::zero(),
                nonce: U256::zero(),
            };

            let mut state = new_state().unwrap();

            state
                .add_balance(
                    &sender.address,
                    balance,
                    ethcore::state::CleanupMode::NoEmpty,
                )
                .unwrap();

            state.commit().unwrap();
            let (root, mut db) = state.drop();
            db.0.commit().unwrap();

            miner::mine_block(None, root);

            sender
        }

        fn create_contract(&mut self, code: Vec<u8>, balance: &U256) -> Address {
            let contract = contract_address(
                vm::CreateContractAddress::FromCodeHash,
                &self.address,
                &self.nonce,
                &code,
            ).0;

            let tx = Transaction {
                action: Action::Create,
                value: *balance,
                data: code,
                gas: U256::max_value(),
                gas_price: U256::zero(),
                nonce: self.nonce,
            }.fake_sign(self.address);

            let (_exec, root) = execute_transaction(&tx).unwrap();
            miner::mine_block(Some(tx.hash()), root);
            self.nonce += U256::one();

            contract
        }

        fn call(&mut self, contract: &Address, data: Vec<u8>, value: &U256) -> H256 {
            let tx = Transaction {
                action: Action::Call(*contract),
                value: *value,
                data: data,
                gas: U256::max_value(),
                gas_price: U256::zero(),
                nonce: self.nonce,
            }.fake_sign(self.address);

            let (exec, root) = execute_transaction(&tx).unwrap();
            miner::mine_block(Some(tx.hash()), root);
            self.nonce += U256::one();

            H256::from_slice(exec.output.as_slice())
        }
    }

    impl Default for Client {
        fn default() -> Self {
            Self::new(&U256::max_value())
        }
    }

    #[test]
    fn test_create_balance() {
        let init_bal = U256::from(42);
        let contract_bal = U256::from(10);
        let remaining_bal = init_bal - contract_bal;

        let mut client = Client::new(&init_bal);

        let code = hex::decode("3331600055").unwrap(); // SSTORE(0x0, BALANCE(CALLER()))
        let contract = client.create_contract(code, &contract_bal);

        let new_state = get_state().unwrap();

        assert_eq!(new_state.balance(&client.address).unwrap(), remaining_bal);
        assert_eq!(new_state.nonce(&client.address).unwrap(), U256::one());
        assert_eq!(new_state.balance(&contract).unwrap(), contract_bal);
        assert_eq!(
            new_state.storage_at(&contract, &H256::zero()).unwrap(),
            H256::from(&remaining_bal)
        );
    }

    #[test]
    fn test_solidity_blockhash() {
        // contract The {
        //   function hash(uint8 num) public pure returns (uint8) {
        //       return blockhash;
        //     }
        // }

        let mut client = Client::default();

        let blockhash_code = hex::decode("608060405234801561001057600080fd5b5060c78061001f6000396000f300608060405260043610603f576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063cc8ee489146044575b600080fd5b348015604f57600080fd5b50606f600480360381019080803560ff169060200190929190505050608d565b60405180826000191660001916815260200191505060405180910390f35b60008160ff164090509190505600a165627a7a72305820349ccb60d12533bc99c8a927d659ee80298e4f4e056054211bcf7518f773f3590029").unwrap();

        let contract = client.create_contract(blockhash_code, &U256::zero());

        let mut blockhash = |num: u8| -> H256 {
            let mut data = hex::decode(
                "cc8ee4890000000000000000000000000000000000000000000000000000000000000000",
            ).unwrap();
            data[35] = num;
            client.call(&contract, data, &U256::zero())
        };

        assert_ne!(blockhash(0), H256::zero());
        assert_ne!(blockhash(2), H256::zero());
        assert_eq!(blockhash(5), H256::zero());
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

        let mut client = Client::default();

        let contract_a_code = hex::decode("608060405234801561001057600080fd5b5061015d806100206000396000f3006080604052600436106100405763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663e3f300558114610045575b600080fd5b34801561005157600080fd5b5061007673ffffffffffffffffffffffffffffffffffffffff60043516602435610088565b60408051918252519081900360200190f35b6000808390508073ffffffffffffffffffffffffffffffffffffffff1663346fb5c9846040518263ffffffff167c010000000000000000000000000000000000000000000000000000000002815260040180828152602001915050602060405180830381600087803b1580156100fd57600080fd5b505af1158015610111573d6000803e3d6000fd5b505050506040513d602081101561012757600080fd5b50519493505050505600a165627a7a7230582062a004e161bd855be0a78838f92bafcbb4cef5df9f9ac673c2f7d174eff863fb0029").unwrap();
        let contract_a = client.create_contract(contract_a_code, &U256::zero());

        let contract_b_code = hex::decode("6080604052348015600f57600080fd5b50609c8061001e6000396000f300608060405260043610603e5763ffffffff7c0100000000000000000000000000000000000000000000000000000000600035041663346fb5c981146043575b600080fd5b348015604e57600080fd5b506058600435606a565b60408051918252519081900360200190f35b600101905600a165627a7a72305820ea09447c835e5eb442e1a85e271b0ae6decf8551aa73948ab6b53e8dd1fa0dca0029").unwrap();
        let contract_b = client.create_contract(contract_b_code, &U256::zero());

        let data = hex::decode(format!(
            "e3f30055000000000000000000000000{:\
             x}0000000000000000000000000000000000000000000000000000000000000029",
            contract_b
        )).unwrap();
        let output = client.call(&contract_a, data, &U256::zero());

        assert_eq!(output, H256::from(42));
    }
}
