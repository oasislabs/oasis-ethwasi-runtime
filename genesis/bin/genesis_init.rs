//! A utility for generating oasis-node's genesis state for oasis-runtime.
#![deny(warnings)]

extern crate clap;
extern crate ethcore;
extern crate io_context;
extern crate oasis_core_runtime;
extern crate oasis_runtime_common;
extern crate serde_json;

use std::{fs::File, sync::Arc};

use clap::{crate_authors, crate_version, App, Arg};
use ethcore::spec::Spec;
use io_context::Context;
use oasis_core_runtime::storage::{
    mkvs::{urkel::sync::NoopReadSyncer, UrkelTree},
    StorageContext,
};
use oasis_runtime_common::{
    parity::NullBackend,
    storage::{MemoryKeyValue, ThreadLocalMKVS},
};

fn main() {
    let matches = App::new("Genesis state generator")
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("eth_genesis")
                .help("Ethereum genesis file")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("output_file")
                .help("Oasis storage genesis file")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    // Load Ethereum genesis state.
    let eth_genesis = File::open(matches.value_of("eth_genesis").unwrap()).unwrap();
    let spec = Spec::load(eth_genesis).expect("failed to load Ethereum genesis file");

    // Populate MKVS with state required at genesis.
    let untrusted_local = Arc::new(MemoryKeyValue::new());
    let mut mkvs = UrkelTree::make()
        .with_capacity(0, 0)
        .new(Box::new(NoopReadSyncer {}));

    StorageContext::enter(&mut mkvs, untrusted_local, || {
        spec.ensure_db_good(
            Box::new(ThreadLocalMKVS::new(Context::background())),
            NullBackend,
            &Default::default(),
        )
        .expect("genesis initialization must succeed");
    });

    let (write_log, _) = mkvs
        .commit(Context::background(), Default::default(), 0)
        .expect("mkvs commit must succeed");

    // Serialize genesis state.
    let mut output = File::create(matches.value_of("output_file").unwrap()).unwrap();
    serde_json::to_writer_pretty(&mut output, &write_log).unwrap();
}
