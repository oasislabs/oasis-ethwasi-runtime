extern crate ekiden_tools;

fn main() {
    // not using protobufs
    //ekiden_tools::generate_mod("src/generated", &["api"]);
    ekiden_tools::build_api();
}
