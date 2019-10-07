fn main() {
    let mut client = runtime_ethereum::test::Client::new();
    loop {
        honggfuzz::fuzz!(|params: (
            Vec<u8>,
            [u8; 32],
            Option<u64>,
            Option<bool>,
            Vec<u8>,
            [u8; 32]
        )| {
            let (code, balance, expiry, c10lity, call_data, call_value) = params;
            let addr =
                match client.create_contract_with_header(code, &balance.into(), expiry, c10lity) {
                    Ok((_hash, addr)) => addr,
                    Err(_) => return,
                };
            client.call(&addr, call_data, &call_value.into()).ok();
        });
    }
}
