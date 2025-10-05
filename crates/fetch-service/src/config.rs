use derive_more::Constructor;
use std::net::SocketAddr;

// fetch service configuration
#[derive(Constructor, Debug)]
pub struct FetchServiceConfig {
    // fetch service address to bind
    pub addr: SocketAddr,
}
