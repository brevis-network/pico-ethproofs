use common::utils::addr_to_url;
use reqwest::Url;
use std::{net::SocketAddr, sync::Arc};

// number of mock subblock grpc services
pub const NUM_MOCK_PROVING_SUBBLOCKS: usize = 7;

// mock proving aggregator address
pub const MOCK_PROVING_AGGREGATOR_ADDR: &str = "[::1]:55551";

// mock proving subblock address (use the same address for multiple mock proving subblock services)
pub const MOCK_PROVING_SUBBLOCK_ADDR: &str = "[::1]:55552";

// mock emulation cycles
pub const MOCK_CYCLES: u64 = 1234;

// seconds of mock proving time
pub const MOCK_PROVING_MILLISECONDS: u64 = 10_000;

// mock proof bytes
// TODO: read from dump file if necessary for verification
pub const MOCK_PROOF: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

// mock proving service configuration
#[derive(Debug)]
pub struct MockProvingServiceConfig {
    // maximum grpc message bytes
    pub max_msg_bytes: usize,

    // proof service grpc address for returning the mock proof
    pub proof_service_url: Url,
}

impl MockProvingServiceConfig {
    pub fn new(max_msg_bytes: usize, proof_service_addr: &SocketAddr) -> Arc<Self> {
        let proof_service_url = addr_to_url(proof_service_addr, "http://");

        Self {
            max_msg_bytes,
            proof_service_url,
        }
        .into()
    }
}
