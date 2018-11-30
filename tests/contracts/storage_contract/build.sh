cargo build --target wasm32-unknown-unknown --release

target_dir=${CARGO_TARGET_DIR:-target}

wasm-build \
  --target wasm32-unknown-unknown \
  --max-mem 4194304 \
  $target_dir \
  storage_contract
