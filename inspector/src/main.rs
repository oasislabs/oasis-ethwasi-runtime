#![deny(warnings)]
extern crate clap;
extern crate ethcore;
extern crate log;
extern crate patricia_trie as parity_patricia_trie;
extern crate pretty_env_logger;

extern crate ekiden_core;
extern crate ekiden_db_trusted;
extern crate ekiden_roothash_base;
extern crate ekiden_roothash_client;
extern crate ekiden_storage_base;
extern crate ekiden_storage_batch;
extern crate ekiden_storage_client;

extern crate runtime_ethereum_common;

use std::{io::Cursor, str::FromStr, sync::Arc};

use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use ethcore::{
    blockchain::BlockChain,
    spec::Spec,
    state::{backend::Wrapped as WrappedBackend, Backend},
};
use log::{info, LevelFilter};

use ekiden_core::{
    environment::{Environment, GrpcEnvironment},
    futures::{block_on, prelude::*},
};
use ekiden_db_trusted::{patricia_trie::PatriciaTrie, Database, DatabaseHandle};
use ekiden_roothash_base::RootHashBackend;
use ekiden_roothash_client::RootHashClient;
use ekiden_storage_base::BackendIdentityMapper;
use runtime_ethereum_common::{get_factories, BlockchainStateDb, StorageHashDB};

fn main() {
    // Initialize logger.
    pretty_env_logger::formatted_builder()
        .unwrap()
        .filter(None, LevelFilter::Debug)
        .init();

    let args = App::new(concat!(crate_name!(), " client"))
        .about(crate_description!())
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("runtime_id")
                .long("runtime_id")
                .takes_value(true)
                .default_value("0000000000000000000000000000000000000000000000000000000000000000")
                .required(true)
                .help("Runtime identifier"),
        )
        .arg(
            Arg::with_name("state_root")
                .long("state_root")
                .takes_value(true)
                .help("Optional state root to use instead of requesting through the runtime"),
        )
        .args(&ekiden_core::remote_node::get_arguments())
        .get_matches();

    // Initialize storage and database overlays.
    let environment: Arc<Environment> = Arc::new(GrpcEnvironment::default());
    let remote_node = ekiden_core::remote_node::RemoteNode::from_args(&args);
    let channel = remote_node.create_channel(environment.clone());
    let roothash = Arc::new(RootHashClient::new(channel.clone()));
    let raw_storage = Arc::new(ekiden_storage_client::StorageClient::new(channel));
    let storage = Arc::new(ekiden_storage_batch::BatchStorageBackend::new(raw_storage));
    let mut db = DatabaseHandle::new(storage.clone());

    // Configure the state root.
    let state_root;
    if let Some(state_root_str) = args.value_of("state_root") {
        state_root =
            ekiden_core::bytes::H256::from_str(state_root_str).expect("state root must be valid");

        info!("Using configured Ekiden state root: {:?}", state_root);
    } else {
        // Fetch the latest Ekiden block.
        let runtime_id = args.value_of("runtime_id").expect("runtime_id is required");
        let runtime_id =
            ekiden_core::bytes::B256::from_str(runtime_id).expect("runtime id must be valid");

        info!("Fetching latest block for runtime {:?}", runtime_id);
        let blk = block_on(
            environment,
            roothash
                .get_blocks(runtime_id)
                .into_future()
                .map(|(item, _)| item)
                .map_err(|(error, _)| error),
        );
        let blk = blk
            .expect("roothash block must be available")
            .expect("roothash block must exist");

        // Configure state root.
        info!("Found Ekiden state root: {:?}", blk.header.state_root);
        state_root = blk.header.state_root;
    }

    db.set_root_hash(state_root)
        .expect("root hash set must succeed");

    // Iterate over the Ekiden patricia trie.
    let ekiden_trie = PatriciaTrie::new(Arc::new(BackendIdentityMapper::new(storage.clone())));
    let stats = ekiden_trie.stats(Some(state_root), usize::max_value());
    info!("Ekiden trie statistics: {:?}", stats);

    // Initialize state with genesis block.
    let blockchain_db = Arc::new(BlockchainStateDb::new(db));
    let state_db = StorageHashDB::new(storage.clone(), blockchain_db.clone());

    info!("Initializing Ethereum genesis block");
    // TODO: Make this configurable via CLI.
    let genesis_json = include_str!("../../resources/genesis/genesis.json");
    let spec = Spec::load(Cursor::new(genesis_json)).unwrap();
    let state_backend = spec
        .ensure_db_good(WrappedBackend(Box::new(state_db.clone())), &get_factories())
        .expect("state to be initialized");
    state_db.commit();

    // Initialize the blockchain index.
    let chain = BlockChain::new(
        Default::default(), /* config */
        &spec.genesis_block(),
        blockchain_db.clone(),
    );
    let eth_blk = chain.best_block_header();
    info!("Found latest Ethereum block header: {:?}", eth_blk);

    // Iterate over the Parity patricia trie(s).
    let trie = parity_patricia_trie::TrieDB::new(state_backend.as_hashdb(), &eth_blk.state_root())
        .unwrap();
    let stats = trie.stats();
    info!("Ethereum state trie statistics: {:?}", stats);
}
