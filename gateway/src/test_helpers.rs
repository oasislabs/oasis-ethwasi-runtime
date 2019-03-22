use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use clap::{App, Arg};
use ekiden_common::bytes::H256;
use ekiden_core::{error::Result, futures::prelude::*};
use ekiden_db_trusted::{Database, DatabaseHandle};
use ekiden_keymanager_common::StateKeyType;
use ekiden_storage_base::{InsertOptions, StorageBackend};
use ekiden_storage_dummy::DummyStorageBackend;
use ethcore::{encoded, filter::TxEntry, ids::BlockId};
use ethereum_types;
use hex;
use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};

use client::ChainNotify;
use runtime_ethereum;

pub struct MockNotificationHandler {
    headers: Mutex<Vec<encoded::Header>>,
    log_notifications: Mutex<Vec<(BlockId, BlockId)>>,
}

impl MockNotificationHandler {
    pub fn new() -> Self {
        Self {
            headers: Mutex::new(vec![]),
            log_notifications: Mutex::new(vec![]),
        }
    }

    pub fn get_notified_headers(&self) -> Vec<encoded::Header> {
        let headers = self.headers.lock().unwrap();
        headers.clone()
    }

    pub fn get_log_notifications(&self) -> Vec<(BlockId, BlockId)> {
        let notifications = self.log_notifications.lock().unwrap();
        notifications.clone()
    }
}

impl ChainNotify for MockNotificationHandler {
    fn has_heads_subscribers(&self) -> bool {
        true
    }

    fn notify_heads(&self, headers: &[encoded::Header]) {
        let mut existing = self.headers.lock().unwrap();
        for &ref header in headers {
            existing.push(header.clone());
        }
    }

    fn notify_logs(&self, from_block: BlockId, to_block: BlockId) {
        let mut notifications = self.log_notifications.lock().unwrap();
        notifications.push((from_block, to_block));
    }

    fn notify_completed_transaction(&self, entry: &TxEntry, output: Vec<u8>) {}
}

pub fn get_test_runtime_client() -> runtime_ethereum::Client {
    let args = App::new("testing")
        .arg(
            Arg::with_name("mr-enclave")
                .long("mr-enclave")
                .takes_value(true)
                .default_value("0000000000000000000000000000000000000000000000000000000000000000"),
        )
        .arg(
            Arg::with_name("node-address")
                .long("node-address")
                .takes_value(true)
                .default_value("127.0.0.1:42261"),
        )
        .get_matches();

    runtime_client!(runtime_ethereum, args)
}

#[derive(Deserialize)]
pub struct MockDb {
    #[serde(deserialize_with = "deserialize_db")]
    db: BTreeMap<Vec<u8>, Vec<u8>>,
    #[serde(deserialize_with = "deserialize_storage")]
    storage: Arc<StorageBackend>,
}

struct DbVisitor {}

impl DbVisitor {
    fn new() -> Self {
        Self {}
    }
}

impl<'de> Visitor<'de> for DbVisitor {
    type Value = BTreeMap<Vec<u8>, Vec<u8>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("db map")
    }

    fn visit_map<M>(self, mut access: M) -> core::result::Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = BTreeMap::new();
        while let Some((key, value)) = access.next_entry::<String, String>()? {
            map.insert(
                hex::decode(key).unwrap().to_vec(),
                hex::decode(value).unwrap().to_vec(),
            );
        }
        Ok(map)
    }
}

struct StorageVisitor {}

impl StorageVisitor {
    fn new() -> Self {
        Self {}
    }
}

impl<'de> Visitor<'de> for StorageVisitor {
    type Value = Arc<StorageBackend>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("storage list")
    }

    fn visit_seq<S>(self, mut seq: S) -> core::result::Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let storage = Arc::new(DummyStorageBackend::new());
        while let Some(value) = seq.next_element()? {
            let hex: String = value;
            storage
                .insert(hex::decode(hex).unwrap(), 10, InsertOptions::default())
                .wait()
                .unwrap();
        }
        Ok(storage)
    }
}

pub fn deserialize_db<'de, D>(
    deserializer: D,
) -> core::result::Result<BTreeMap<Vec<u8>, Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(DbVisitor::new())
}

pub fn deserialize_storage<'de, D>(
    deserializer: D,
) -> core::result::Result<Arc<StorageBackend>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(StorageVisitor::new())
}

impl MockDb {
    pub fn empty() -> Self {
        Self {
            storage: Arc::new(DummyStorageBackend::new()),
            db: BTreeMap::new(),
        }
    }

    pub fn new() -> Self {
        let json = include_str!("../resources/mockdb.json");
        serde_json::from_str(&json).unwrap()
    }

    pub fn storage(&self) -> Arc<StorageBackend> {
        self.storage.clone()
    }
}

impl Database for MockDb {
    fn contains_key(&self, key: &[u8]) -> bool {
        self.db.contains_key(key)
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.db.get(key).cloned()
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.db.insert(key.to_vec(), value.to_vec())
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.db.remove(key)
    }

    fn set_root_hash(&mut self, _root_hash: H256) -> Result<()> {
        unimplemented!();
    }

    fn get_root_hash(&self) -> H256 {
        unimplemented!();
    }

    fn commit(&mut self) -> Result<H256> {
        unimplemented!();
    }

    fn rollback(&mut self) {
        self.db.clear()
    }

    fn with_encryption<F, R>(&mut self, _contract_id: H256, _f: F) -> R
    where
        F: FnOnce(&mut DatabaseHandle) -> R,
    {
        unimplemented!();
    }

    fn with_encryption_key<F, R>(&mut self, _key: StateKeyType, _f: F) -> R
    where
        F: FnOnce(&mut DatabaseHandle) -> R,
    {
        unimplemented!();
    }
}
