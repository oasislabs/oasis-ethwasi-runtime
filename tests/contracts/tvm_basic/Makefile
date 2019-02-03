.PHONY: all clean

AR ?= llvm-ar

all: target/tvm_basic.wasm

target/tvm_basic.wasm: target/wasm32-unknown-unknown/release/tvm_basic.wasm
	wasm-build --target=wasm32-unknown-unknown --max-mem=1048576 target/ tvm_basic_contract

target/wasm32-unknown-unknown/release/tvm_basic.wasm: target/tvm/libtest.a
	cargo build --target=wasm32-unknown-unknown --release

target/tvm/libtest.a: target/tvm/add.o
	$(AR) r $@ $^

target/tvm/add.o: src/prepare_test_libs.py | target/tvm
	python3 $< -o $(@D)

target/tvm:
	@mkdir -p $@

clean:
	cargo clean
