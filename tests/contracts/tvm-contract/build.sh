cargo build --target wasm32-unknown-unknown --release
wasm-build --target wasm32-unknown-unknown --stack-size 1048576 ./target tvm_contract