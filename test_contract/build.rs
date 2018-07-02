use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=BAR_LIB_DIR");

    let lib_dir = env::var("BAR_LIB_DIR").unwrap();
    /*
    let out_dir = env::var("OUT_DIR").unwrap();

    let in_path: PathBuf = [&lib_dir, "bar.o"].iter().collect();
    let out_path: PathBuf = [&lib_dir, "libbar.a"].iter().collect();
    let mut builder = Builder::new(File::create(out_path.to_str().unwrap()).unwrap());
    builder.append_path(in_path.to_str().unwrap()).unwrap();
    */

    println!("cargo:rustc-link-search=native={}", lib_dir);
    println!("cargo:rustc-link-lib=static=bar");
}
