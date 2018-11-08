cargo build --target wasm32-unknown-unknown --release

target_dir=${CARGO_TARGET_DIR:-target}

wasm-build \
  --target wasm32-unknown-unknown \
  --max-mem 262144 \
  $target_dir \
  rust_logistic_contract
