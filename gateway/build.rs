fn main() {
    let git_rev = String::from_utf8(
        std::process::Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    println!("cargo:rustc-env=CARGO_PKG_GIT_REV={}", &git_rev[..7]);
}
