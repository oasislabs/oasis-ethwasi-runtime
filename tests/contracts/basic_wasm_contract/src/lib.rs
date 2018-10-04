extern crate hex;
extern crate pwasm_ethereum;
extern crate parity_hash;

use parity_hash::H256;

#[no_mangle]
pub fn deploy() {}

#[no_mangle]
pub fn call() {
    pwasm_ethereum::ret(&b"result"[..]);
}
