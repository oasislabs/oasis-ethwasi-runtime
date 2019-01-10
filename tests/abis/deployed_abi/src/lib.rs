extern crate owasm_abi;
extern crate owasm_abi_derive;
extern crate owasm_ethereum;
extern crate owasm_std;
use owasm_abi::types::*;
#[owasm_abi_derive::eth_abi(DeployedRustEndpoint, DeployedRustClient)]
pub trait DeployedRust {
    fn constructor(&mut self);
    #[constant]
    fn a(&mut self) -> U256;
    fn set_a(&mut self, a: U256);
}
