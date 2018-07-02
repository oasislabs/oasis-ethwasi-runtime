//#![feature(proc_macro, wasm_custom_selection, wasm_import_module)]
extern crate bigint;
extern crate parity_hash;
extern crate pwasm_ethereum;
extern crate wee_alloc;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use bigint::U256;
use parity_hash::H256;

extern "C" {
  fn add(a: i32, b: i32) -> i32;
}

#[no_mangle]
pub fn deploy() {
  unsafe { add(4, 5) };
}

#[no_mangle]
pub fn call() {
  //pwasm_ethereum::write(&H256::zero().into(), &U256::one().into());
  //assert_eq!(pwasm_ethereum::read(&H256::zero().into()).into(), U256::one());
  //pwasm_ethereum::ret(&b"success"[..]);
  //pwasm_ethereum::ret(&pwasm_ethereum::read(&H256::zero().into()) as &[u8]);
  //pwasm_ethereum::ret(&b"success"[..]);
  pwasm_ethereum::ret(H256::from(U256::from(unsafe { add(4, 5) })).as_ref());
}
