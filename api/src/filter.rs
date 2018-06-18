use serde::{de, Deserialize, Serialize};
use bigint::{Address, H256};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TopicFilter {
    All,
    Or(Vec<H256>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogFilter {
    pub from_block: Option<String>,
    pub to_block: Option<String>,
    pub addresses: Vec<Address>,
    pub topics: Vec<TopicFilter>,
}
