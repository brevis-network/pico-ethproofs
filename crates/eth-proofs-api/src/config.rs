use derive_more::Constructor;
use reqwest::Url;
use std::{fs, path::PathBuf};

// eth-proofs request content-type
pub const ETH_PROOFS_CONTENT_TYPE: &str = "application/json";

// `queued` notification url path
const QUEUED_URL_PATH: &str = "/api/v0/proofs/queued";

// `proving` notification url path
const PROVING_URL_PATH: &str = "/api/v0/proofs/proving";

// `proved` notification url path
const PROVED_URL_PATH: &str = "/api/v0/proofs/proved";

// uint32 array size of subblock verification key digest
const SUBBLOCK_VK_DIGEST_SIZE: usize = 8;

#[derive(Clone, Constructor, Debug)]
pub struct EthProofsAPIConfig {
    // eth-proofs API URL to report the block proving status
    pub url: Url,

    // eth-proofs API token
    pub token: String,

    // eth-proofs app cluster ID
    pub cluster_id: u64,

    // subblock verification key digest file path; read this file as an unique verifier ID
    pub subblock_vk_digest_path: PathBuf,
}

impl EthProofsAPIConfig {
    // build the authorization header
    pub fn authorization_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    // get the full `queued` notification url path
    pub fn queued_url(&self) -> Url {
        self.url.join(QUEUED_URL_PATH).unwrap()
    }

    // get the full `proving` notification url path
    pub fn proving_url(&self) -> Url {
        self.url.join(PROVING_URL_PATH).unwrap()
    }

    // get the full `proved` notification url path
    pub fn proved_url(&self) -> Url {
        self.url.join(PROVED_URL_PATH).unwrap()
    }

    // get the verifier ID
    pub fn verifier_id(&self) -> String {
        // read and deserialize the subblock verification key digest
        let data = fs::read(&self.subblock_vk_digest_path).expect(
            "eth-proofs-api: failed to read subblock verification key digest from the file",
        );
        let vk_digest: [u32; SUBBLOCK_VK_DIGEST_SIZE] = bincode::deserialize(&data)
            .expect("eth-proofs-api: failed to deserialize subblock verification key digest");

        // encode verification key digest to a hex string
        let vk_digest_bytes: Vec<_> = vk_digest.iter().flat_map(|u| u.to_be_bytes()).collect();
        hex::encode(vk_digest_bytes)
    }
}
