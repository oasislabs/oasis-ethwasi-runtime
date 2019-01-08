#![no_std]

#[owasm_abi_derive::contract]
trait BasicWasm {
    fn constructor(&mut self) {}

    fn my_method(&mut self) -> Vec<u8> {
        (b"result"[..]).to_vec()
    }
}
