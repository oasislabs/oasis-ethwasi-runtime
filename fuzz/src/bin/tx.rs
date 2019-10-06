fn main() {
    let mut client = runtime_ethereum::test::Client::new();
    loop {
        honggfuzz::fuzz!(|params: ([u8; 20], Vec<u8>, [u8; 32])| {
            let (addr, data, value) = params;
            client
                .send(
                    Some(&addr.into()),
                    data,
                    &value.into(),
                    None, /* nonce */
                )
                .ok();
        });
    }
}
