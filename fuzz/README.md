# Runtime fuzzing infra

The runtime is fuzzed using [honggfuzz](https://github.com/google/honggfuzz) via [cargo hfuzz](https://crates.io/crates/honggfuzz).

Available fuzzing targets are in `src/bin/*.rs`.
Currently there are two targets:
* `create_contract`, which creates a new contract and, if that succeeds, calls it
* `tx`, which sends random transactions to random addresses

To begin fuzzing,

1. install the hfuzz [dependencies](https://github.com/rust-fuzz/honggfuzz-rs#dependencies)
2. `cargo install honggfuzz`
3. ensure that runtime is buildable (i.e. build or otherwise obtain Ekiden enclaves)
4. `./run.sh <target name>`

At the current time, both targets have been fuzzed for over 1m iterations with no crashes found.
While encouraging, don't let this stop you from trying!
