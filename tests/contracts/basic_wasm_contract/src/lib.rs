#![no_std]

#[owasm_abi_derive::contract]
trait BasicWasm {
    fn constructor(&mut self) {}

    fn my_method(&mut self) {
        owasm_ethereum::ret(&b"result"[..]);
    }
}
