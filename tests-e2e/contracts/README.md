# Test Contracts

This directory contains contracts used in tests.

## Building WASM Contracts

To build WASM contracts, you need to add the `wasm32-unknown-unknown` Rust target and
the `wasm-build` utility. You can do both by running:
```
rustup target add wasm32-unknown-unknown
cargo install --force --git https://github.com/oasislabs/wasm-utils.git --branch ekiden
```

After that you can build the WASM contracts by running the corresponding `build.sh`
script.
