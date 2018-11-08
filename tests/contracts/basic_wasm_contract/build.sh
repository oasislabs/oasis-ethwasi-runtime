cargo build --target wasm32-unknown-unknown --release

target_dir=${CARGO_TARGET_DIR:-target}
echo "target_dir = $target_dir"

wasm-build \
  --target wasm32-unknown-unknown \
  --max-mem 4194304 \
  $target_dir \
  basic_contract
