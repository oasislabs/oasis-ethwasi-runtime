use std::io::Cursor;

use ethereum_api::{BlockId as EkidenBlockId, Log};

use ethcore::ids::BlockId;
use ethcore::spec::Spec;
use ethereum_types::U256;

pub fn gwei_to_wei(gwei: u64) -> U256 {
    U256::from(gwei).saturating_mul(U256::from(1_000_000_000))
}

pub fn load_spec() -> Spec {
    #[cfg(not(feature = "benchmark"))]
    let spec_json = include_str!("../../resources/genesis/genesis.json");
    #[cfg(feature = "benchmark")]
    let spec_json = include_str!("../../resources/genesis/genesis_benchmarking.json");
    Spec::load(Cursor::new(spec_json)).unwrap()
}

pub fn from_block_id(id: BlockId) -> EkidenBlockId {
    match id {
        BlockId::Number(number) => EkidenBlockId::Number(number.into()),
        BlockId::Hash(hash) => EkidenBlockId::Hash(hash),
        BlockId::Earliest => EkidenBlockId::Earliest,
        BlockId::Latest => EkidenBlockId::Latest,
    }
}
