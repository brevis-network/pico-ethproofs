use crate::subblock_executor::SubblockExecutor;
use anyhow::Result;
use common::report::BlockProvingReport;
use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgSender, FetchMsg, FetchMsgReceiver, ProvingMsg};
use std::{sync::Arc, time::Instant};
use tokio::{spawn, sync::Mutex, task::JoinHandle};
use tracing::{error, info};

// sub block fetcher for fetching blocks by a start block number and a count specified requested
// number of blocks
#[derive(Constructor)]
pub struct ProvingFromStartFetcher {
    // receiving fetch messages
    fetch_receiver: Arc<Mutex<FetchMsgReceiver>>,

    // sending proving messages to the proving-client thread
    proving_sender: Arc<BlockMsgSender>,

    // executor for generating subblock and aggregation inputs
    subblock_executor: Arc<SubblockExecutor>,
}

impl ProvingFromStartFetcher {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("proving-from-start-fetcher: start");

        spawn(async move {
            let mut fetch_receiver = self.fetch_receiver.lock().await;
            while let Some(msg) = fetch_receiver.recv().await {
                match msg {
                    FetchMsg::ProveFromStart {
                        start_block_number,
                        count,
                    } => {
                        info!(
                            "proving-from-start-fetcher: received from-start fetch message of start_block_number = {start_block_number}, count = {count}",
                        );
                        for block_number in start_block_number..start_block_number + count {
                            info!(
                                "proving-from-start-fetcher: starting for fetching block {block_number}"
                            );
                            if let Err(e) = self.fetch_block(block_number).await {
                                error!(
                                    "proving-from-start-fetcher: failed to fetch block-{block_number} {e:?}",
                                );
                            }
                            info!(
                                "proving-from-start-fetcher: succeeded for fetching block {block_number}",
                            );
                        }
                    }
                    _ => error!("proving-from-start-fetcher: received a wrong message {msg:?}"),
                }
            }
        })
    }

    // fetch a specified block by number
    async fn fetch_block(&self, block_number: u64) -> Result<()> {
        // generate proving inputs of the specified block number
        let start_time = Instant::now();
        let proving_inputs = self.subblock_executor.generate_inputs(block_number).await?;
        let data_fetch_milliseconds = start_time.elapsed().as_millis() as u64;

        // create a block report
        let fetch_report = BlockProvingReport::new(block_number, data_fetch_milliseconds);

        // send the proving message
        let msg = BlockMsg::Proving(ProvingMsg::new(fetch_report, proving_inputs));
        self.proving_sender.send(msg)?;

        Ok(())
    }
}
