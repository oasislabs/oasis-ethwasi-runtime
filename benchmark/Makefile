SHELL = /bin/bash

all: build

# Build the benchmark.
build:
	@echo "Building benchmark"
	@go build -v -o ./benchmark

clean: clean-go

clean-go:
	@go clean

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
