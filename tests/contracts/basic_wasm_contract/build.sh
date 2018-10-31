cargo build --target wasm32-unknown-unknown --release
wasm-build --target wasm32-unknown-unknown --max-mem 4194304 ../../../target basic_contract
