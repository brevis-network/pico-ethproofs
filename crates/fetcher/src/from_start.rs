use crate::subblock_executor::SubblockExecutor;
use anyhow::Result;
use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgSender, FetchMsg, FetchMsgReceiver};
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};
use tracing::{error, info};

// sub block fetcher for fetching blocks by a start block number and a count specified requested
// number of blocks
#[derive(Constructor)]
pub struct FromStartFetcher {
    // receiving fetch messages
    fetch_receiver: Arc<FetchMsgReceiver>,

    // sending proving messages to the proving-client thread
    proving_sender: Arc<BlockMsgSender>,

    // executor for generating subblock and aggregation inputs
    subblock_executor: Arc<SubblockExecutor>,
}

impl FromStartFetcher {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("from-start-fetcher: start");

        spawn(async move {
            while let Ok(msg) = self.fetch_receiver.recv() {
                match msg {
                    FetchMsg::FromStart {
                        start_block_number,
                        count,
                    } => {
                        info!(
                            "from-start-fetcher: received from-start fetch message of start_block_number = {start_block_number}, count = {count}",
                        );
                        for block_number in start_block_number..start_block_number + count {
                            info!("from-start-fetcher: starting for fetching block {block_number}");
                            if let Err(e) = self.fetch_block(block_number).await {
                                error!(
                                    "from-start-fetcher: failed to fetch block-{block_number} {e:?}",
                                );
                            }
                            info!(
                                "from-start-fetcher: succeeded for fetching block {block_number}",
                            );
                        }
                    }
                    _ => error!("from-start-fetcher: received a wrong message {msg:?}"),
                }
            }
        })
    }

    // fetch a specified block by number
    async fn fetch_block(&self, block_number: u64) -> Result<()> {
        // generate proving inputs of the specified block number
        let proving_inputs = self.subblock_executor.generate_inputs(block_number).await?;

        // send the proving message
        let msg = BlockMsg::Proving(proving_inputs);
        self.proving_sender.send(msg)?;

        Ok(())
    }
}
