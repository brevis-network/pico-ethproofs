use crate::{config::BlockFetcherConfig, subblock_executor::SubblockExecutor};
use alloy_provider::{Provider, ProviderBuilder, WsConnect};
use anyhow::Result;
use common::report::BlockProvingReport;
use crossbeam::channel::TryRecvError;
use derive_more::Constructor;
use futures::StreamExt;
use messages::{BlockMsg, BlockMsgSender, FetchMsg, FetchMsgReceiver, ProvingMsg};
use std::{sync::Arc, time::Instant};
use tokio::{spawn, task::JoinHandle};
use tracing::{error, info};

// maximum fetch number of blocks in each batch
const NUM_BLOCKS_PER_BATCH: usize = 10;

// sub block fetcher for fetching the latest blocks by a count specified requested number of blocks
#[derive(Constructor)]
pub struct LatestFetcher {
    // fetcher configuration
    config: Arc<BlockFetcherConfig>,

    // receiving fetch messages
    fetch_receiver: Arc<FetchMsgReceiver>,

    // sending proving messages to the proving-client thread
    proving_sender: Arc<BlockMsgSender>,

    // executor for generating subblock and aggregation inputs
    subblock_executor: Arc<SubblockExecutor>,
}

impl LatestFetcher {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("latest-fetcher: start");

        spawn(async move {
            loop {
                // save the processed fetch number in the current batch
                let mut batch_fetch_count = 0;

                // save the total remaining number of latest blocks
                let mut remaining_count = 0;

                // handle latest block fetch message and update remaining count if necessary
                let new_count = if remaining_count == 0 {
                    info!(
                        "latest-fetcher: waiting for a request fetch number for the latest blocks",
                    );

                    match self.fetch_receiver.recv() {
                        Ok(FetchMsg::Latest { count }) => count,
                        msg => {
                            error!(
                                "latest-fetcher: fetch receiver received an unexpected message {msg:?}",
                            );
                            break;
                        }
                    }
                } else {
                    info!(
                        "latest-fetcher: try to receive a new fetch number for the latest blocks",
                    );
                    match self.fetch_receiver.try_recv() {
                        Ok(FetchMsg::Latest { count }) => count,
                        Err(TryRecvError::Empty) => {
                            // received no message and return the same remaining count
                            remaining_count
                        }
                        msg => {
                            error!(
                                "latest-fetcher: fetch receiver received an unexpected message {msg:?}",
                            );
                            break;
                        }
                    }
                };

                // set the remaining count to the maximum value compared with new request
                remaining_count = remaining_count.max(new_count);
                info!(
                    "latest-fetcher: received latest fetch message of count {new_count} and update remaining count to {remaining_count}",
                );

                if remaining_count == 0 {
                    // unnecessary to subscribe to latest block since no fetch number is requested
                    continue;
                }

                // initialize a websocket rpc connection for receiving latest blocks
                let ws_conn = WsConnect::new(self.config.rpc_ws_url.as_str());
                let provider = ProviderBuilder::new()
                    .connect_ws(ws_conn)
                    .await
                    .expect("latest-fetcher: failed to connect to rpc websocket URL");
                let subscription = provider
                    .subscribe_blocks()
                    .await
                    .expect("latest-fetcher: failed to subscribe the latest blocks");
                let mut latest_block_receiver = subscription.into_stream();

                // handle the new block notification from the websocket rpc
                while let Some(header) = latest_block_receiver.next().await {
                    let block_number = header.number;
                    info!(
                        "latest-fetcher: rpc websocket connection received a new block {block_number}",
                    );

                    if let Err(e) = self.fetch_block(block_number).await {
                        error!("latest-fetcher: failed to fetch block-{block_number} {e:?}",);
                    }
                    info!("latest-fetcher: succeeded for fetching block {block_number}");

                    batch_fetch_count += 1;
                    remaining_count -= 1;

                    // exit the current fetching batch if no remaining blocks or reaching the
                    // maximum number of blocks per batch
                    if remaining_count == 0 || batch_fetch_count >= NUM_BLOCKS_PER_BATCH {
                        break;
                    }
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
