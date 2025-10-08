use crate::{config::BlockFetcherConfig, subblock_executor::SubblockExecutor};
use alloy_provider::{Provider, ProviderBuilder, WsConnect};
use anyhow::Result;
use common::report::BlockProvingReport;
use derive_more::Constructor;
use futures::StreamExt;
use messages::{BlockMsg, BlockMsgSender, FetchMsg, FetchMsgReceiver, ProvingMsg};
use std::{sync::Arc, time::Instant};
use tokio::{
    select, spawn,
    task::{JoinHandle, spawn_blocking},
    time::{Duration, sleep},
};
use tracing::{error, info};

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
            // initialize a websocket rpc connection for receiving latest blocks
            let ws_conn = WsConnect::new(self.config.rpc_ws_url.as_str());
            let provider = ProviderBuilder::new().connect_ws(ws_conn).await.unwrap();

            // subscribe to latest block
            let subscription = provider.subscribe_blocks().await.unwrap();
            let mut latest_block_receiver = subscription.into_stream();

            // save the current remaining number of latest blocks
            let mut remaining_count = 0;

            loop {
                select! {
                    biased;

                    // handle latest block fetch message and update remaining count if necessary
                    // TODO: fix to asynchronous channel for avoiding `spawn_blocking`
                    msg = spawn_blocking({
                        let receiver = self.fetch_receiver.clone();
                        move || receiver.recv()
                    }) => {
                        match msg {
                            Ok(Ok(FetchMsg::Latest { count })) => {
                                // set the maximum value to the current remaining count
                                remaining_count = remaining_count.max(count);

                                info!(
                                    "latest-fetcher: received latest fetch message of count = {count} and update remaining count to {remaining_count}",
                                );
                            }
                            _ => {
                                error!("latest-fetcher: fetch receiver received an unexpected message {msg:?}");
                                break;
                            }
                        }
                    }

                    // handle the new block notification from the websocket rpc
                    header = latest_block_receiver.next() => {
                        match header {
                            Some(header) => {
                                let block_number = header.number;
                                info!("latest-fetcher: websocket rpc received new block {block_number}");

                                if remaining_count > 0 {
                                    info!("latest-fetcher: starting for fetching block {block_number}");
                                    if let Err(e) = self.fetch_block(block_number).await {
                                        error!(
                                            "latest-fetcher: failed to fetch block-{block_number} {e:?}",
                                        );
                                    }

                                    remaining_count -= 1;
                                    info!(
                                        "latest-fetcher: succeeded for fetching block {block_number} and decrease remaining count to {remaining_count}",
                                    );
                                }
                            }
                            None => {
                                info!("latest-fetcher: websocket rpc received none of new blocks");
                                sleep(Duration::from_secs(1)).await;
                            }
                        }
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
