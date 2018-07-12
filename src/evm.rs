use std::{cmp, collections::BTreeMap, sync::Arc};

use ekiden_core::error::Result;
use ethcore::{
  executive::{contract_address, Executed, Executive, TransactOptions},
  machine::EthereumMachine,
  spec::CommonParams,
  transaction::{SignedTransaction, Transaction},
  vm,
};
use ethereum_types::{Address, H256, U256};

use super::state::{get_block_hash, get_latest_block_number, get_state, with_state};

/// as per https://github.com/paritytech/parity/blob/master/ethcore/res/ethereum/byzantium_test.json
macro_rules! evm_params {
  () => {{
    let mut params = CommonParams::default();
    params.maximum_extra_data_size = 0x20;
    params.min_gas_limit = 0x1388.into();
    params.network_id = 0x01;
    params.max_code_size = 49152;
    params.eip98_transition = <u64>::max_value();
    params.gas_limit_bound_divisor = 0x0400.into();
    params.registrar = "0xc6d9d2cd449a754c494264e1809c50e34d64562b".into();
    params.wasm_activation_transition = 1;
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
  println!(
    "machine schedule: {:?}",
    machine.schedule(get_env_info().number).create_data_limit
  );

  with_state(|state| {
    Ok(Executive::new(state, &get_env_info(), &machine)
      .transact(&transaction, TransactOptions::with_no_tracing())?)
  })
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
  use super::{
    super::{miner, state, util::strip_0x},
    *,
  };

  use std::{
    default::Default,
    fs::File,
    io::{self, prelude::*},
    str,
  };

  use ethcore::{
    self,
    executive::contract_address,
    transaction::{Action, Transaction},
  };
  use ethereum_types::Address;
  use hex;

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

      let mut state = get_state().unwrap();

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

      let (exec, root) = execute_transaction(&tx).unwrap();
      println!("{:?}", exec);
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
      match exec.exception {
        Some(err) => panic!(err),
        None => println!("No exception"),
      }
      println!("exec: {:?}", exec);

      let output = str::from_utf8(exec.output.as_ref()).unwrap();
      //println!("output: {}", output);
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

  #[test]
  fn test_tvm() {
    let mut client = Client::new(&U256::from("10000000"));
    let state = get_state().unwrap();
    println!("{:?}", state.balance(&client.address).unwrap());

    //let mut file = match File::open("../wasm-rust/target/wasm_rust.wasm")
    let mut file = match File::open("test_contract/target/test_contract.wasm")
    //let mut file = match File::open("../pwasm-tutorial/step-0/target/pwasm_tutorial_contract.wasm")
    //let mut file = match File::open("/home/ec2-user/wasm-explorations/3-Rust+C-nostd/foo/target/foo.wasm")
    {
      Err(why) => panic!(why),
      Ok(file) => file,
    };

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer);

    //let contract_code = hex::decode("0061736d01000000010d0360027f7f0060017f0060000002270303656e7603726574000003656e760673656e646572000103656e76066d656d6f727902010110030201020404017000000501000708010463616c6c00020901000ac10101be0102057f017e4100410028020441c0006b22043602042004412c6a41106a220041003602002004412c6a41086a22014200370200200441186a41106a22024100360200200441186a41086a220342003703002004420037022c2004410036021c20044100360218200441186a1001200020022802002202360200200120032903002205370200200441106a2002360200200441086a200537030020042004290318220537022c200420053703002004411410004100200441c0006a3602040b0b0a010041040b0410c00000").unwrap();

    let contract = client.create_contract(buffer, &U256::from(10));
    println!("contract created");
    let output = client.call(&contract, Vec::new(), &U256::zero());

    /*
    let new_state = get_state().unwrap();
    println!("{:?}", new_state.balance(&client.address).unwrap());
    println!("{:?}", new_state.balance(&contract).unwrap())
    */
    /*
    assert_eq!(
      new_state.storage_at(&contract, &H256::zero()).unwrap(),
      H256::from(U256::one())
    );
    */

    println!("{:?}", output);
    //assert_eq!(output, H256::from_slice(&b"success"[..]));
    assert_eq!(output, H256::from(U256::from(9i32)));
  }

  #[test]
  fn test_sterling() {
    let mut client = Client::new(&U256::from("1000000"));
    let state = get_state().unwrap();

    let mut provider_file = match File::open("../marketplace/provider/target/provider.wasm") {
      Err(why) => panic!(why),
      Ok(file) => file,
    };

    let mut provider_buffer = Vec::new();
    provider_file.read_to_end(&mut provider_buffer);

    let mut consumer_file = match File::open("../marketplace/consumer/target/consumer.wasm") {
      Err(why) => panic!(why),
      Ok(file) => file,
    };

    let mut consumer_buffer = Vec::new();
    consumer_file.read_to_end(&mut consumer_buffer);

    let provider_contract = client.create_contract(provider_buffer, &U256::from(10));
    let output = client.call(&provider_contract, "write_data()".into(), &U256::zero());
    //assert_eq!(&*output, "write_data(data)".as_bytes());
  }
}
