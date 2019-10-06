repo_root=$(git rev-parse --show-toplevel)

export KM_ENCLAVE_PATH="$repo_root/.ekiden/target/x86_64-fortanix-unknown-sgx/debug/ekiden-keymanager-runtime.sgxs"
export RUSTFLAGS='-Ctarget-feature=+aes,+ssse3'

cd "$repo_root/fuzz"

cargo hfuzz run $1
