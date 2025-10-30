use crate::config::{ETH_PROOFS_CONTENT_TYPE, EthProofsAPIConfig};
use anyhow::anyhow;
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Client, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use std::sync::Arc;
use tracing::error;

// maximum number of retries for sending requests
const MAX_REQUEST_RETRIES: u32 = 5;

// request content-type header key
const CONTENT_TYPE_HEADER_KEY: &str = "Content-Type";

// request authorization header key
const AUTHORIZATION_HEADER_KEY: &str = "Authorization";

#[derive(Debug)]
pub struct EthProofsClient {
    // eth-proofs API configuration
    config: EthProofsAPIConfig,

    // eth-proofs request client
    client: ClientWithMiddleware,

    // unique verifier ID used as a request argument
    verifier_id: String,
}

impl EthProofsClient {
    pub fn new(config: EthProofsAPIConfig) -> Arc<Self> {
        // build the request client
        let retry_policy =
            ExponentialBackoff::builder().build_with_max_retries(MAX_REQUEST_RETRIES);
        let client = ClientBuilder::new(Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        // get the verifier ID
        let verifier_id = config.verifier_id();

        Self {
            config,
            client,
            verifier_id,
        }
        .into()
    }

    // report the block proving task is queued
    // <https://ethproofs.org/api.html#tag/Proofs/paths/~1proofs~1queued/post>
    pub async fn queued(self: Arc<Self>, block_number: u64) {
        let url = self.config.queued_url();

        let request = serde_json::json!({
            "block_number": block_number,
            "cluster_id": self.config.cluster_id,
        });

        self.post_request(url, request).await;
    }

    // report the block proving task is proving
    // <https://ethproofs.org/api.html#tag/Proofs/paths/~1proofs~1proving/post>
    pub async fn proving(self: Arc<Self>, block_number: u64) {
        let url = self.config.proving_url();

        let request = serde_json::json!({
            "block_number": block_number,
            "cluster_id": self.config.cluster_id,
        });

        self.post_request(url, request).await;
    }

    // report the block proving task is complete
    // <https://ethproofs.org/api.html#tag/Proofs/paths/~1proofs~1proved/post>
    pub async fn proved(
        self: Arc<Self>,
        block_number: u64,
        proving_cycles: u64,
        proving_milliseconds: u64,
        proof_bytes: &[u8],
    ) {
        let url = self.config.proved_url();

        let request = serde_json::json!({
            "proof": STANDARD.encode(proof_bytes),
            "block_number": block_number,
            "proving_cycles": proving_cycles,
            "proving_time": proving_milliseconds,
            "verifier_id": self.verifier_id,
            "cluster_id": self.config.cluster_id,
        });

        self.post_request(url, request).await;
    }

    // send a common post request to eth-proofs
    pub async fn post_request(self: Arc<Self>, url: Url, request: serde_json::Value) {
        tokio::spawn(async move {
            let response = self
                .client
                .post(url)
                .header(CONTENT_TYPE_HEADER_KEY, ETH_PROOFS_CONTENT_TYPE)
                .header(AUTHORIZATION_HEADER_KEY, self.config.authorization_header())
                .json(&request)
                .send()
                .await
                .map_err(|e| anyhow!("eth-proofs-api: request error {e:?}"))
                .and_then(|res| {
                    res.error_for_status()
                        .map_err(|e| anyhow!("eth-proofs-api: response error {e:?}"))
                });

            if let Err(e) = response {
                error!("eth-proofs-api: failed to send a post request {e:?}");
            }
        });
    }
}
