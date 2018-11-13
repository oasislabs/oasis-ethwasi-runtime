fn main() {
    // Required step to link the TVM module.
    println!("cargo:rustc-link-search=native={}", "tvm_module");
}
