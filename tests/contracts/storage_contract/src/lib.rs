extern crate hex;
extern crate pwasm_ethereum;
extern crate parity_hash;

use parity_hash::H256;
//use std::panic;

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    //panic::set_hook(Box::new(|panic_info| println!("{}", panic_info)));
    let key = H256::from_slice(&hex::decode("416ca9bc81f92551a25be4a2a33fe68f97299a280c038ff99af267ad59c99aeb").unwrap());
    let retrieved = pwasm_ethereum::fetch_bytes(&key, 5).unwrap();
    //println!("retrieved: {:?}", retrieved);
    pwasm_ethereum::ret(&retrieved);
}
