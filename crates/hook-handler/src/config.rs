use derive_more::Constructor;
use eth_proofs_api::config::EthProofsAPIConfig;

// proving hook handler configuration
#[derive(Constructor, Debug)]
pub struct HookHandlerConfig {
    // eth-proofs API configuration; report to eth-proofs if it's set
    pub eth_proofs_config: Option<EthProofsAPIConfig>,
}
