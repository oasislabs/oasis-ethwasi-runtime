cargo build --target wasm32-unknown-unknown --release
wasm-build --target wasm32-unknown-unknown --max-mem 262144 ../../../target rust_logistic_contract
