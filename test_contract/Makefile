OUTFILES := target/test_contract.wasm

DOCKER_IMAGE_VERSION := 0.2.0
DOCKER_IMAGE := wehlutyk/wasm-compiler:$(DOCKER_IMAGE_VERSION)
export BAR_LIB_DIR := libbar
DOCKER_RUN = docker run \
  --user $(shell id -u):$(shell id -g) \
  --volume $(CURDIR):/c:ro \
  --volume $(CURDIR)/$(BAR_LIB_DIR):/c/$(BAR_LIB_DIR) \
  --workdir /c \
  --interactive \
  --tty \
  --rm \
  $(DOCKER_IMAGE)
#--volume $(CURDIR)/target:/c/target \
#--volume $HOME/.cargo:/cargo \
#--volume (rustc +nightly --print sysroot):/rust:ro \

all: $(OUTFILES)

$(OUTFILES): target/wasm32-unknown-unknown/release/test_contract.wasm
	wasm-build --target=wasm32-unknown-unknown ./target test_contract

target/wasm32-unknown-unknown/release/test_contract.wasm: src/* $(BAR_LIB_DIR)/libbar.a
	cargo build --release --target=wasm32-unknown-unknown

$(BAR_LIB_DIR)/libbar.a: $(BAR_LIB_DIR)/bar.o
	llvm-ar rcsv $(BAR_LIB_DIR)/libbar.a $(BAR_LIB_DIR)/bar.o

$(BAR_LIB_DIR)/bar.o: $(BAR_LIB_DIR)/bar.c
	clang -c --target=wasm32-unknown-unknown-wasm -o $(BAR_LIB_DIR)/bar.o $(BAR_LIB_DIR)/bar.c

try:
	clang --target=wasm32-unknown-unknown -S -o $(BAR_LIB_DIR)/bar.s $(BAR_LIB_DIR)/bar.c
	llvm-as -o $(BAR_LIB_DIR)/bar.o

clean:
	cargo clean
	rm -f $(BAR_LIB_DIR)/bar.o $(BAR_LIB_DIR)/libbar.a
	rm -rf $(DIST_BROWSER) $(DIST_NODEJS)

blankslate: clean
	rm -f Cargo.lock
	rm -rf node_modules
	rm -f package-lock.json

env:
	npm install

test: $(OUTPATHS_NODEJS)
	node run.js | grep "Hello, World!!"
	node run.js | grep "4 + 5 = 9"

.PHONY: all clean env blankslate test
