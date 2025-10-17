use derive_more::Constructor;
use reqwest::Url;
use std::path::PathBuf;

// proving client configuration
#[derive(Constructor, Debug)]
pub struct ProvingClientConfig {
    // maximum grpc message bytes
    pub max_msg_bytes: usize,

    // aggregator proving grpc urls
    pub agg_url: Url,

    // subbblock proving grpc urls
    pub subblock_urls: Vec<Url>,

    // file path of serialized subblock verification key digest
    pub subblock_vk_digest_path: PathBuf,
}
