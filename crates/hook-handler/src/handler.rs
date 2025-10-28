use crate::config::HookHandlerConfig;
use eth_proofs_api::client::EthProofsClient;
use messages::{BlockMsg, BlockMsgReceiver, HookMsg};
use std::sync::Arc;
use tokio::{spawn, sync::Mutex, task::JoinHandle};
use tracing::{error, info};

// proving hook handler
#[derive(Debug)]
pub struct HookHandler {
    // communication receiver for coordinating with the main scheduler
    comm_receiver: Arc<Mutex<BlockMsgReceiver>>,

    // eth-proofs client for reporting the proving status if it's set
    eth_proofs_client: Option<Arc<EthProofsClient>>,
}

impl HookHandler {
    pub fn new(config: HookHandlerConfig, comm_receiver: Arc<Mutex<BlockMsgReceiver>>) -> Self {
        let eth_proofs_client = config.eth_proofs_config.map(EthProofsClient::new);

        Self {
            comm_receiver,
            eth_proofs_client,
        }
    }

    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("hook-handler: start");

        spawn(async move {
            let mut comm_receiver = self.comm_receiver.lock().await;
            while let Some(msg) = comm_receiver.recv().await {
                match msg {
                    BlockMsg::Hook(hook_msg) => match hook_msg {
                        HookMsg::FetchStart { block_number } => {
                            info!("hook-handler: start to fetch block {block_number}");

                            if let Some(client) = self.eth_proofs_client.clone() {
                                info!(
                                    "hook-handler: notify block {block_number} queued to eth-proofs",
                                );

                                client.queued(block_number).await;
                            }
                        }
                        HookMsg::FetchEnd { block_number } => {
                            info!("hook-handler: finish fetching block {block_number}");
                        }
                        HookMsg::ProveStart { block_number } => {
                            info!("hook-handler: start to prove block {block_number}");

                            if let Some(client) = self.eth_proofs_client.clone() {
                                info!(
                                    "hook-handler: notify block {block_number} proving to eth-proofs",
                                );

                                client.proving(block_number).await;
                            }
                        }
                        HookMsg::ProveEnd {
                            block_number,
                            cycles,
                            proving_milliseconds,
                            proof,
                        } => {
                            info!("hook-handler: finish proving block {block_number}");

                            if let Some(client) = self.eth_proofs_client.clone() {
                                info!(
                                    "hook-handler: notify block {block_number} proved to eth-proofs",
                                );

                                client
                                    .proved(block_number, cycles, proving_milliseconds, &proof)
                                    .await;
                            }
                        }
                    },
                    _ => error!("hook-handler: received a wrong message {msg:?}"),
                }
            }
            info!("hook-handler: stopped");
        })
    }
}
