extern crate ekiden_edl;
extern crate ekiden_tools;

use std::env;

fn main() {
    ekiden_tools::build_trusted(ekiden_edl::edl());
    generate_km_enclave_identity();
}

/// Take a previously built key manager enclave and generate an identity file.
/// The runtime uses this to extract the key manager's MRENCLAVE at compile time
/// via the use_key_manager_contract! macro.
fn generate_km_enclave_identity() {
    let km_enclave_path = env::var("KM_ENCLAVE_PATH").expect("Please define KM_ENCLAVE_PATH");
    ekiden_tools::generate_mod("src/generated", &[]);
    ekiden_tools::generate_enclave_identity(
        "src/generated/ekiden-key-manager.identity",
        &km_enclave_path,
    );
}
