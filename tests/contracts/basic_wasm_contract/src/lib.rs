extern crate hex;
extern crate parity_hash;
extern crate pwasm_ethereum;

use parity_hash::H256;

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    pwasm_ethereum::ret(&b"result"[..]);
}
