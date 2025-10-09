use crate::config::{MockProvingServiceConfig, NUM_MOCK_PROVING_SUBBLOCKS};
use common::utils::addr_to_url;
use derive_more::Constructor;
use reqwest::Url;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

// fetch http and websocket service
#[derive(Constructor, Debug)]
pub struct MockProvingService {
    pub config: Arc<MockProvingServiceConfig>,
}

impl MockProvingService {
    // return mock aggregator grpc url
    pub fn aggregator_url(&self) -> Url {
        addr_to_url(&self.aggregator_addr(), "http://")
    }

    // return mock subblock grpc urls
    pub fn subblock_urls(&self) -> Vec<Url> {
        let url = addr_to_url(&self.subblock_addr(), "http://");

        vec![url; NUM_MOCK_PROVING_SUBBLOCKS]
    }

    pub fn run(self: Arc<Self>) -> Vec<JoinHandle<()>> {
        info!("mock-proving-service: start");

        let agg_handle = self.clone().run_aggregator_service();
        let subblock_handle = self.run_subblock_service();

        vec![agg_handle, subblock_handle]
    }
}
