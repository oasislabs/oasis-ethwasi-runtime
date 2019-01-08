#![no_std]

extern crate hex;

#[owasm_abi_derive::contract]
trait Storage {
    fn constructor(&mut self) {}

    fn get(&mut self) -> Vec<u8> {
        let key = H256::from_slice(&hex::decode(
            "416ca9bc81f92551a25be4a2a33fe68f97299a280c038ff99af267ad59c99aeb",
        ).unwrap());
        owasm_ethereum::fetch_bytes(&key, 5).unwrap()
    }
}
