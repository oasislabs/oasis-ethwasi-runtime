#![no_std]

extern crate hex;

#[owasm_abi_derive::contract]
trait Storage {
    fn constructor(&mut self) {}

    fn get(&mut self) {
        //panic::set_hook(Box::new(|panic_info| println!("{}", panic_info)));
        let key = H256::from_slice(&hex::decode(
            "416ca9bc81f92551a25be4a2a33fe68f97299a280c038ff99af267ad59c99aeb",
        ).unwrap());
        let retrieved = owasm_ethereum::fetch_bytes(&key, 5).unwrap();
        //println!("retrieved: {:?}", retrieved);
        owasm_ethereum::ret(&retrieved);
    }
}
