SHELL = /bin/bash

# GOPATH used in the Ekiden development Docker container.
DOCKER_GOPATH ?= /go
# Code directory used in the Ekiden development Docker container.
DOCKER_CODE_DIR ?= $(shell pwd)/..
# runtime-ethereum under GOPATH.
DOCKER_OASISLABS_GOPATH ?= $(DOCKER_GOPATH)/src/github.com/oasislabs
DOCKER_RUNTIME_ETHEREUM_GOPATH ?= $(DOCKER_OASISLABS_GOPATH)/runtime-ethereum

all: prepare dep build

# Prepare environment (if we are in Ekiden Docker container).
prepare:
	@set -e; \
	if [[ "$$GOPATH" == $(DOCKER_GOPATH) && -d $(DOCKER_CODE_DIR) ]]; then \
		echo "Preparing Go environment"; \
		if [[ ! -d $(DOCKER_OASISLABS_GOPATH) ]]; then \
			mkdir -p $(DOCKER_OASISLABS_GOPATH); \
			ln -s -f $(DOCKER_CODE_DIR) $(DOCKER_RUNTIME_ETHEREUM_GOPATH); \
		fi; \
	fi;

# Ensure all dependencies are present.
dep:
	@echo "Ensuring package dependencies"
	@test -d $(DOCKER_RUNTIME_ETHEREUM_GOPATH) && cd $(DOCKER_RUNTIME_ETHEREUM_GOPATH)/benchmark; \
	dep ensure -v

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
	@test -d $(DOCKER_RUNTIME_ETHEREUM_GOPATH) && cd $(DOCKER_RUNTIME_ETHEREUM_GOPATH)/benchmark; \
	go test -race -v ./...

.PHONY: all prepare dep build lint test
