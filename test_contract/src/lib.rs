extern crate pwasm_ethereum;
extern crate parity_hash;
extern crate bigint;
extern crate wee_alloc;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use parity_hash::H256;
use bigint::U256;

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    pwasm_ethereum::write(&H256::zero().into(), &U256::one().into());
    //assert_eq!(pwasm_ethereum::read(&H256::zero().into()).into(), U256::one());
    //pwasm_ethereum::ret(&b"success"[..]);
    pwasm_ethereum::ret(&pwasm_ethereum::read(&H256::zero().into()) as &[u8]);
}
