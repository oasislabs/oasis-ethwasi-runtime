SHELL = /bin/bash
CONTRACTS = readwrite 

all: build

# Build the benchmark.
build: contracts
	@echo "Building benchmark"
	@go build -v -o ./benchmark

# Build contract to bytecode and generate abi and generate go bindings with
# abigen
contracts: FORCE
	@for c in $(CONTRACTS); do \
		cd contracts/$$c; \
		cargo build --release --target wasm32-unknown-unknown && \
		wasm-build --target=wasm32-unknown-unknown ./target $$c && \
		export CONTRACT_CLASS=`basename target/json/*.json .json` && \
		xxd -p target/$$c.wasm > target/$$c.wasm.hex && \
		abigen --bin=target/$$c.wasm.hex --abi=target/json/$$CONTRACT_CLASS.json --pkg=$$c --out=$$CONTRACT_CLASS.go; \
	done

clean: clean-go clean-contracts

clean-go:
	@go clean

clean-contracts:
	@for c in $(CONTRACTS); do \
		cd contracts/$$c; \
		cargo clean; \
		export CONTRACT_CLASS=`basename target/json/*.json .json`; \
		rm -f $$CONTRACT_CLASS.go; \
	done

# Run go fmt.
fmt:
	go fmt ./...

# Lint.
lint:
	golangci-lint run

# Test.
test:
	go test -race -v ./...

FORCE:

.PHONY: all build lint test
