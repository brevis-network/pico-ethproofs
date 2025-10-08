use derive_more::Constructor;
use reqwest::Url;

// proving client configuration
#[derive(Constructor, Debug)]
pub struct ProvingClientConfig {
    // maximum grpc message bytes
    pub max_msg_bytes: usize,

    // aggregator proving grpc urls
    pub agg_url: Url,

    // subbblock proving grpc urls
    pub subblock_urls: Vec<Url>,
}
