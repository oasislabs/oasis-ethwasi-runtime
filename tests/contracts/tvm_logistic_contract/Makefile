.PHONY: all clean

all: target/tvm_logistic_contract.wasm

target/tvm_logistic_contract.wasm: target/wasm32-unknown-unknown/release/tvm_logistic_contract.wasm
	wasm-build --target=wasm32-unknown-unknown --max-mem=36801012 target tvm_logistic_contract

target/wasm32-unknown-unknown/release/tvm_logistic_contract.wasm: tvm_module/libtest.a
	cargo build --target=wasm32-unknown-unknown --release

tvm_module/libtest.a: tvm_module/credit_scoring.o
	llvm-ar r tvm_module/libtest.a tvm_module/credit_scoring.o

tvm_module/credit_scoring.o: src/model.py
	mkdir -p tvm_module
	python3 src/model.py tvm_module

clean:
	cargo clean
	rm -rf tvm_module
