SHELL = /bin/bash

all: build

# Build the benchmark.
build:
	@echo "Building benchmark"
	@go build -v -o ./benchmark

# Run go fmt.
fmt:
	go fmt ./...

# Lint.
lint:
	golangci-lint run

# Test.
test:
	go test -race -v ./...

.PHONY: all build lint test
