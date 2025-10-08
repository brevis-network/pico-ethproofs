use derive_more::Constructor;
use std::net::SocketAddr;

// proof grpc service configuration
#[derive(Constructor, Debug)]
pub struct ProofServiceConfig {
    // proof grpc service address
    pub addr: SocketAddr,

    // maximum grpc message bytes
    pub max_msg_bytes: usize,
}
