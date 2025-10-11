use reqwest::Url;
use std::net::SocketAddr;

// maximum number of subblocks for proving
pub const MAX_NUM_SUBBLOCKS: usize = 7;

// convert a socket address to an url
// - addr: socket address
// - scheme_prefix: url scheme prefix , e.g. `http://` or `https://`
pub fn addr_to_url(addr: &SocketAddr, scheme_prefix: &str) -> Url {
    Url::parse(&format!("{scheme_prefix}{addr}"))
        .expect("failed to convert a socket address to an URL")
}
