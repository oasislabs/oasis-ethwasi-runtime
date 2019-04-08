use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use ekiden_client::{Node, TxnClient};
use ekiden_runtime::{
    common::crypto::hash::Hash,
    storage::{cas::MemoryCAS, CAS, MKVS},
};
use ethcore::{encoded, filter::TxEntry, ids::BlockId};
use failure::Fallible;
use grpcio::EnvBuilder;
use hex;
extern crate serde;
use test_helpers::serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};

use crate::{client::ChainNotify, EthereumRuntimeClient};

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

    fn notify_completed_transaction(&self, _entry: &TxEntry, _output: Vec<u8>) {}
}

pub fn new_test_runtime_client() -> EthereumRuntimeClient {
    let env = Arc::new(EnvBuilder::new().build());
    let node = Node::new(env.clone(), "invalid-address");
    let txn_client = TxnClient::new(node.channel(), Default::default(), None);
    EthereumRuntimeClient::new(txn_client)
}

#[derive(Deserialize)]
pub struct MockDb {
    #[serde(deserialize_with = "deserialize_mkvs")]
    mkvs: BTreeMap<Vec<u8>, Vec<u8>>,
    #[serde(deserialize_with = "deserialize_cas")]
    cas: Arc<CAS>,
}

struct MKVSVisitor {}

impl MKVSVisitor {
    fn new() -> Self {
        Self {}
    }
}

impl<'de> Visitor<'de> for MKVSVisitor {
    type Value = BTreeMap<Vec<u8>, Vec<u8>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("mkvs map")
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

struct CASVisitor {}

impl CASVisitor {
    fn new() -> Self {
        Self {}
    }
}

impl<'de> Visitor<'de> for CASVisitor {
    type Value = Arc<CAS>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("cas list")
    }

    fn visit_seq<S>(self, mut seq: S) -> core::result::Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let cas = Arc::new(MemoryCAS::new());
        while let Some(value) = seq.next_element()? {
            let hex: String = value;
            cas.insert(hex::decode(hex).unwrap(), 10).unwrap();
        }
        Ok(cas)
    }
}

pub fn deserialize_mkvs<'de, D>(
    deserializer: D,
) -> core::result::Result<BTreeMap<Vec<u8>, Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(MKVSVisitor::new())
}

pub fn deserialize_cas<'de, D>(deserializer: D) -> core::result::Result<Arc<CAS>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_seq(CASVisitor::new())
}

impl MockDb {
    pub fn empty() -> Self {
        Self {
            cas: Arc::new(MemoryCAS::new()),
            mkvs: BTreeMap::new(),
        }
    }

    pub fn new() -> Self {
        let json = include_str!("../resources/mockdb.json");
        serde_json::from_str(&json).unwrap()
    }

    pub fn cas(&self) -> Arc<CAS> {
        self.cas.clone()
    }
}

impl MKVS for MockDb {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.mkvs.get(key).cloned()
    }

    fn insert(&mut self, key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
        self.mkvs.insert(key.to_vec(), value.to_vec())
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.mkvs.remove(key)
    }

    fn commit(&mut self) -> Fallible<Hash> {
        unimplemented!();
    }

    fn rollback(&mut self) {
        self.mkvs.clear()
    }

    fn set_encryption_key(&mut self, _key: Option<&[u8]>) {
        unimplemented!();
    }
}
