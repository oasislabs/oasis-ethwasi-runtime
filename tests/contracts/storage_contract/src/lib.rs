extern crate hex;
extern crate pwasm_ethereum;
extern crate parity_hash;

use parity_hash::H256;
use std::panic;

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    panic::set_hook(Box::new(|panic_info| println!("{}", panic_info)));
    let mut key = H256::from_slice(&hex::decode("416ca9bc81f92551a25be4a2a33fe68f97299a280c038ff99af267ad59c99aeb").unwrap());
    let mut retrieved = pwasm_ethereum::request_bytes(key, 5).unwrap();
    println!("retrieved: {:?}", retrieved);
    //let bytes = vec![1, 2, 3, 4, 5];
    //key = pwasm_ethereum::store_bytes(&bytes, 5).unwrap();
    //retrieved = pwasm_ethereum::request_bytes(key, 5).unwrap();
    pwasm_ethereum::ret(&retrieved);
}
