.PHONY: all clean

all: target/tvm_basic_contract.wasm

target/tvm_basic_contract.wasm: target/wasm32-unknown-unknown/release/tvm_basic_contract.wasm
	wasm-build --target=wasm32-unknown-unknown --max-mem=1048576 target tvm_basic_contract

target/wasm32-unknown-unknown/release/tvm_basic_contract.wasm: tvm_module/libtest.a
	cargo build --target=wasm32-unknown-unknown --release

tvm_module/libtest.a: tvm_module/test_add_one.o
	llvm-ar r tvm_module/libtest.a tvm_module/test_add_one.o

tvm_module/test_add_one.o: src/prepare_test_libs.py
	mkdir -p tvm_module
	python3 src/prepare_test_libs.py tvm_module

clean:
	cargo clean
	rm -rf tvm_module
