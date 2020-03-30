//! A utility for importing genesis state for Ethereum playback benchmarks.
#![deny(warnings)]

extern crate clap;
extern crate ethcore;
extern crate ethereum_types;
extern crate filebuffer;
extern crate hex;
#[macro_use]
extern crate serde_derive;
extern crate grpcio;
extern crate io_context;
extern crate oasis_core_client;
extern crate oasis_core_runtime;
extern crate oasis_runtime_common;
extern crate serde_bytes;
extern crate serde_json;

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Cursor,
    str::FromStr,
    sync::Arc,
};

use clap::{crate_authors, crate_version, value_t_or_exit, App, Arg};
use ethcore::{spec::Spec, state::State};
use ethereum_types::{Address, H256, U256};
use grpcio::{CallOption, EnvBuilder};
use io_context::Context;
use oasis_core_client::{transaction::api::storage, Node};
use oasis_core_runtime::{
    common::{
        crypto::{
            hash::Hash,
            signature::{PublicKey, Signature, SignatureBundle},
        },
        registry, roothash,
    },
    storage::{
        mkvs::{sync::NoopReadSyncer, Tree},
        StorageContext,
    },
};
use oasis_runtime_common::{
    parity::NullBackend,
    storage::{MemoryKeyValue, ThreadLocalMKVS},
};
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

fn main() {
    let matches = App::new("Genesis state import for Ethereum benchmark utility")
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
                .help("Oasis roothash runtime states genesis file in JSON format")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("runtime-id")
                .help("Target runtime ID")
                .long("runtime-id")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("node-address")
                .help("Storage node address")
                .long("node-address")
                .short("a")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("commit-every")
                .help("Commit every so many imported accounts")
                .long("commit-every")
                .takes_value(true)
                .default_value("10000"),
        )
        .get_matches();

    let node_address = matches.value_of("node-address").unwrap();
    let commit_every = value_t_or_exit!(matches, "commit-every", usize);
    let runtime_id = value_t_or_exit!(matches, "runtime-id", roothash::Namespace);

    // Initialize connection to the storage node.
    let env = Arc::new(EnvBuilder::new().build());
    let node = Node::new(env, node_address);
    let storage = storage::StorageClient::new(node.channel());

    let untrusted_local = Arc::new(MemoryKeyValue::new());
    let mut mkvs = Tree::make()
        .with_capacity(0, 0)
        .new(Box::new(NoopReadSyncer {}));

    // Load Ethereum genesis state.
    let genesis_json = include_str!("../../resources/genesis/genesis_testing.json");
    let spec = Spec::load(Cursor::new(genesis_json)).expect("failed to load Ethereum genesis file");

    StorageContext::enter(&mut mkvs, untrusted_local, || {
        // Initialize state with genesis block.
        spec.ensure_db_good(
            Box::new(ThreadLocalMKVS::new(Context::background())),
            NullBackend,
            &Default::default(),
        )
        .expect("genesis initialization must succeed");

        // Initialize Ethereum state access functions.
        let mut state = State::from_existing(
            Box::new(ThreadLocalMKVS::new(Context::background())),
            NullBackend,
            U256::zero(),       /* account_start_nonce */
            Default::default(), /* factories */
            None,
        )
        .expect("state initialization must succeed");

        let mut root = Hash::empty_hash();
        let mut commit = |state: &mut State<_>| {
            state.commit().unwrap();

            let (write_log, state_root) = StorageContext::with_current(|mkvs, _untrusted_local| {
                mkvs.commit(Context::background(), runtime_id, 0)
                    .expect("mkvs commit must succeed")
            });

            // Push to storage.
            storage
                .apply(
                    &storage::ApplyRequest {
                        namespace: runtime_id,
                        src_round: 0,
                        src_root: root,
                        dst_round: 0,
                        dst_root: state_root,
                        writelog: write_log,
                    },
                    CallOption::default().wait_for_ready(true /* wait_for_ready */),
                )
                .expect("storage apply must succeed");

            root = state_root;
        };

        // Iteratively parse input and import into state.
        println!("Injecting accounts...");
        let state_path = matches.value_of("exported_state").unwrap();
        let state_fb = filebuffer::FileBuffer::open(state_path).unwrap();
        let accounts = StateParser::new(&state_fb);
        let mut index = 0;

        for (addr, account) in accounts {
            let address = Address::from_str(strip_0x(&addr)).unwrap();
            let balance = U256::from_str(strip_0x(&account.balance)).unwrap();
            let nonce = U256::from_str(strip_0x(&account.nonce)).unwrap();

            // Inject account.
            // (storage expiry initialized to 0)
            state.new_contract(&address, balance, nonce, 0);
            if let Some(code) = account.code {
                state.init_code(&address, from_hex(&code)).unwrap();
            }

            // Inject account storage items.
            if let Some(storage) = account.storage {
                for (key, value) in storage {
                    let key = H256::from_str(strip_0x(&key)).unwrap();
                    let value = H256::from_str(strip_0x(&value)).unwrap();

                    state.set_storage(&address, key, value).unwrap();
                }
            }

            index += 1;
            if index % commit_every == 0 {
                println!("Imported {} accounts, committing state.", index);
                commit(&mut state);
            }
        }

        commit(&mut state);
    });

    let (_, state_root) = mkvs
        .commit(Context::background(), runtime_id, 0)
        .expect("mkvs commit must succeed");
    println!("Done, genesis state root is {:?}.", state_root);

    // Generate genesis roothash block file.
    let rtg = registry::RuntimeGenesis {
        state_root: state_root,
        state: vec![],
        storage_receipts: vec![SignatureBundle {
            // public_key must not be None, but empty.
            public_key: Some(PublicKey::default()),
            signature: Signature::default(),
        }],
        round: 0,
    };
    let rtg_map: HashMap<roothash::Namespace, registry::RuntimeGenesis> =
        [(runtime_id, rtg)].iter().cloned().collect();

    // Save to file.
    let mut file = File::create(matches.value_of("output_file").unwrap()).unwrap();
    serde_json::to_writer(&mut file, &rtg_map).unwrap();
}
