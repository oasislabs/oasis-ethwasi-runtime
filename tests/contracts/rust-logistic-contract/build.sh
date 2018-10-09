cargo build --target wasm32-unknown-unknown --release
wasm-build --target wasm32-unknown-unknown --stack-size 87108864 ./target rust_logistic_contract
