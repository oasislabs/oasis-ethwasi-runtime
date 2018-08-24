cargo build --target wasm32-unknown-unknown --release
wasm-build --target wasm32-unknown-unknown --stack-size 4194304 ./target storage_contract
