#![deny(warnings)]
extern crate clap;
extern crate ethcore;
extern crate ethereum_types;
extern crate filebuffer;
extern crate hex;
#[macro_use]
extern crate serde_derive;
extern crate ekiden_client;
extern crate ekiden_runtime;
extern crate failure;
extern crate grpcio;
extern crate runtime_ethereum_common;
extern crate serde_bytes;
extern crate serde_json;

use std::{collections::BTreeMap, fs::File, io::Cursor, str::FromStr, sync::Arc};

use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use ekiden_client::{transaction::api::storage, Node};
use ekiden_runtime::{
    common::{crypto::hash::Hash, roothash},
    storage::{cas::PassthroughCAS, mkvs::CASPatriciaTrie, CAS},
};
use ethcore::{
    block::{IsBlock, OpenBlock},
    blockchain::{BlockChain, ExtrasInsert},
    engines::ForkChoice,
    kvdb::{self, KeyValueDB},
    spec::Spec,
    state::backend::Wrapped as WrappedBackend,
};
use ethereum_types::{Address, H256, U256};
use failure::Fallible;
use grpcio::EnvBuilder;
use runtime_ethereum_common::{get_factories, BlockchainStateDb, StorageHashDB, BLOCK_GAS_LIMIT};
use serde_bytes::ByteBuf;
use serde_json::{de::SliceRead, StreamDeserializer};

#[derive(Deserialize)]
struct ExportedAccount {
    balance: String,
    nonce: String,
    code: Option<String>,
    storage: Option<BTreeMap<String, String>>,
}

fn strip_0x(hex: &str) -> &str {
    if hex.starts_with("0x") {
        hex.get(2..).unwrap()
    } else {
        hex
    }
}

fn from_hex<S: AsRef<str>>(hex: S) -> Vec<u8> {
    hex::decode(strip_0x(hex.as_ref())).expect("input should be valid hex-encoding")
}

const EXPORTED_STATE_START: &[u8] = b"{ \"state\": {";
const EXPORTED_STATE_ACCOUNT_SEP: &[u8] = b",";
const EXPORTED_STATE_ADDR_SEP: &[u8] = b": ";
const EXPORTED_STATE_END: &[u8] = b"\n}}";

enum StateParsingState {
    /// { "state": {
    ///             ^
    First,
    /// "0x...": {...}
    ///               ^
    Middle,
    /// }}
    ///   ^
    End,
}

/// Streaming parser for Parity's exported state JSON.
/// https://github.com/paritytech/parity-ethereum/blob/v1.9.7/parity/blockchain.rs#L633-L689
struct StateParser<'a> {
    src: &'a [u8],
    state: StateParsingState,
}

impl<'a> StateParser<'a> {
    fn new(src: &'a [u8]) -> Self {
        let (start, rest) = src.split_at(EXPORTED_STATE_START.len());
        assert_eq!(start, EXPORTED_STATE_START);
        Self {
            src: rest,
            state: StateParsingState::First,
        }
    }
}

impl<'a> Iterator for StateParser<'a> {
    type Item = (String, ExportedAccount);

    fn next(&mut self) -> Option<(String, ExportedAccount)> {
        // }}
        //   ^
        if let StateParsingState::End = self.state {
            return None;
        }

        // \n}}
        // --->^
        let (end, rest) = self.src.split_at(EXPORTED_STATE_END.len());
        if end == EXPORTED_STATE_END {
            self.src = rest;
            self.state = StateParsingState::End;
            return None;
        }

        // ...,
        //    >^
        if let StateParsingState::Middle = self.state {
            let (account_sep, rest) = self.src.split_at(EXPORTED_STATE_ACCOUNT_SEP.len());
            assert_eq!(account_sep, EXPORTED_STATE_ACCOUNT_SEP);
            self.src = rest;
        }

        // \n"0x...": {...}
        // -------->^
        let mut de_addr = StreamDeserializer::new(SliceRead::new(self.src));
        let addr = de_addr.next().unwrap().unwrap();
        let (_, rest) = self.src.split_at(de_addr.byte_offset());
        self.src = rest;

        // "0x...": {...}
        //        ->^
        let (addr_sep, rest) = self.src.split_at(EXPORTED_STATE_ADDR_SEP.len());
        assert_eq!(addr_sep, EXPORTED_STATE_ADDR_SEP);
        self.src = rest;

        // "0x...": {...}
        //          ---->^
        let mut de_account = StreamDeserializer::new(SliceRead::new(self.src));
        let account = de_account.next().unwrap().unwrap();
        let (_, rest) = self.src.split_at(de_account.byte_offset());
        self.src = rest;

        self.state = StateParsingState::Middle;
        Some((addr, account))
    }
}

struct RemoteCAS(storage::StorageClient);

impl RemoteCAS {
    fn insert_batch(&self, batch: Vec<(ByteBuf, u64)>) -> Fallible<()> {
        for chunk in batch.chunks(10000) {
            let mut request = storage::InsertBatchRequest::new();
            request.set_items(
                chunk
                    .into_iter()
                    .map(|(value, expiry)| {
                        let mut request = storage::InsertRequest::new();
                        request.set_data(value.clone().into());
                        request.set_expiry(*expiry);

                        request
                    })
                    .collect(),
            );

            self.0.insert_batch(&request)?;
        }

        Ok(())
    }
}

impl CAS for RemoteCAS {
    fn get(&self, key: Hash) -> Fallible<Vec<u8>> {
        let mut request = storage::GetRequest::new();
        request.set_id(key.as_ref().to_vec());

        let response = self.0.get(&request)?;

        Ok(response.data)
    }

    fn insert(&self, _value: Vec<u8>, _expiry: u64) -> Fallible<Hash> {
        unimplemented!("not needed as we only use insert_batch");
    }
}

fn main() {
    let matches = App::new(concat!(crate_name!(), " client"))
        .about(crate_description!())
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("exported_state")
                .help("Exported Ethereum blockchain state in JSON format")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("output_file")
                .help("Resulting roothash genesis block")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("node-address")
                .help("Storage node address")
                .long("node-address")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let node_address = matches.value_of("node-address").unwrap();

    // Initialize CAS and MKVS overlays.
    let env = Arc::new(EnvBuilder::new().build());
    let node = Node::new(env, node_address);
    let remote_cas = Arc::new(RemoteCAS(storage::StorageClient::new(node.channel())));
    let cas = Arc::new(PassthroughCAS::new(remote_cas.clone()));
    let mkvs = CASPatriciaTrie::new(cas.clone(), &Hash::empty_hash());
    let blockchain_db = Arc::new(BlockchainStateDb::new(mkvs));
    let state_db = StorageHashDB::new(cas.clone(), blockchain_db.clone());

    // Initialize state with genesis block.
    println!("Initializing genesis block");
    let genesis_json = include_str!("../../resources/genesis/genesis_testing.json");
    let spec = Spec::load(Cursor::new(genesis_json)).unwrap();
    let state_backend = spec
        .ensure_db_good(WrappedBackend(Box::new(state_db.clone())), &get_factories())
        .expect("state to be initialized");
    state_db.commit();

    // Open a new block.
    let chain = BlockChain::new(
        Default::default(), /* config */
        &spec.genesis_block(),
        blockchain_db.clone(),
    );
    let parent = chain.best_block_header();
    let mut block = OpenBlock::new(
        &*spec.engine,
        get_factories(),
        false,                         /* tracing */
        state_backend.clone(),         /* state_db */
        &parent,                       /* parent */
        Arc::new(vec![parent.hash()]), /* last hashes */
        Address::default(),            /* author */
        U256::from(BLOCK_GAS_LIMIT),   /* block gas limit */
        vec![],                        /* extra data */
        true,                          /* is epoch_begin */
        &mut Vec::new().into_iter(),   /* ancestry */
        None,
    )
    .unwrap();

    // Iteratively parse input and import into state.
    println!("Injecting accounts");
    let state_path = matches.value_of("exported_state").unwrap();
    let state_fb = filebuffer::FileBuffer::open(state_path).unwrap();
    let accounts = StateParser::new(&state_fb);

    for (addr, account) in accounts {
        let address = Address::from_str(strip_0x(&addr)).unwrap();
        let balance = U256::from_str(strip_0x(&account.balance)).unwrap();
        let nonce = U256::from_str(strip_0x(&account.nonce)).unwrap();

        // Inject account.
        // (storage expiry initialized to 0)
        block
            .block_mut()
            .state_mut()
            .new_contract(&address, balance, nonce, 0);
        if let Some(code) = account.code {
            block
                .block_mut()
                .state_mut()
                .init_code(&address, from_hex(&code))
                .unwrap();
        }

        // Inject account storage items.
        if let Some(storage) = account.storage {
            for (key, value) in storage {
                let key = H256::from_str(strip_0x(&key)).unwrap();
                let value = H256::from_str(strip_0x(&value)).unwrap();

                block
                    .block_mut()
                    .state_mut()
                    .set_storage(&address, key, value)
                    .unwrap();
            }
        }
    }

    println!("Injected all state, ready to commit");

    let block = block
        .close_and_lock()
        .seal(&*spec.engine, Vec::new())
        .unwrap();

    // Queue the db operations necessary to insert this block.
    println!("Block sealed, generating storage transactions for commit");
    let mut db_tx = kvdb::DBTransaction::default();
    chain.insert_block(
        &mut db_tx,
        &block.rlp_bytes(),
        block.receipts().to_owned(),
        ExtrasInsert {
            fork_choice: ForkChoice::New,
            is_finalized: true,
            metadata: None,
        },
    );

    // Commit the insert to the in-memory blockchain cache.
    println!("Commit into in-memory blockchain cache");
    chain.commit();
    // Write blockchain updates.
    println!("Writing blockchain update transactions");
    blockchain_db
        .write(db_tx)
        .expect("write blockchain updates");

    // Commit any pending state updates.
    println!("Commit state updates");
    state_db.commit();
    // Commit any blockchain state updates.
    println!("Commit blockchain state updates");
    let state_root = blockchain_db.commit().expect("commit blockchain state");

    println!("Done, genesis state root is {:?}", state_root);

    // Now push everything to underlying storage as this has all been in-memory.
    println!("Pushing batches to storage backend");
    remote_cas.insert_batch(cas.take_inserts()).unwrap();

    // Generate genesis roothash block file.
    let mut block = roothash::Block::default();
    block.header.state_root = state_root;
    // TODO: Take runtime identifier as an argument.
    let blocks = vec![block];

    // Save to file.
    let mut file = File::create(matches.value_of("output_file").unwrap()).unwrap();
    serde_json::to_writer(&mut file, &blocks).unwrap();
}
