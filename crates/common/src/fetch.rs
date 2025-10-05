use derive_more::Constructor;
use serde::Deserialize;
use std::collections::HashMap;

// HTTP Get request path for proving blocks by the specified block number
// It supports two parameters:
// - start_block_num: it specifies the `start` block number to prove
// - count: it's optional and `1` is the default value, it specifies the number of blocks to prove
pub const HTTP_PROVE_BLOCK_BY_NUMBER_PATH: &str = "/prove_block_by_number";

// HTTP Get request path for proving latest blocks
// It supports one parameter:
// - count: it's optional and `1` is the default value, it specifies the number of latest blocks
//   to prove
pub const HTTP_PROVE_LATEST_BLOCK_PATH: &str = "/prove_latest_block";

// HTTP Get `prove_block_by_number` parameters
#[derive(Constructor, Debug, Deserialize)]
pub struct ProveBlockByNumberParams {
    // specifies the `start` block number to prove
    pub start_block_num: u64,

    // specifies the number of blocks to prove
    pub count: Option<u64>,
}

impl ProveBlockByNumberParams {
    // convert to hash map
    pub fn to_hash_map(&self) -> HashMap<&'static str, u64> {
        let mut params = HashMap::new();

        params.insert("start_block_num", self.start_block_num);
        if let Some(count) = self.count {
            params.insert("count", count);
        }

        params
    }
}

// HTTP Get `prove_latest_block` parameters
#[derive(Constructor, Debug, Deserialize)]
pub struct ProveLatestBlockParams {
    // it specifies the number of latest blocks to prove
    pub count: Option<u64>,
}

impl ProveLatestBlockParams {
    // convert to hash map
    pub fn to_hash_map(&self) -> HashMap<&'static str, u64> {
        let mut params = HashMap::new();

        if let Some(count) = self.count {
            params.insert("count", count);
        }

        params
    }
}
