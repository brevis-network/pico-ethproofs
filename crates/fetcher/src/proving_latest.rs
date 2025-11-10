use crate::{config::BlockFetcherConfig, subblock_executor::SubblockExecutor};
use alloy_provider::{Provider, ProviderBuilder, WsConnect};
use alloy_rpc_types::Header;
use anyhow::Result;
use common::report::BlockProvingReport;
use derive_more::Constructor;
use futures::StreamExt;
use messages::{BlockMsg, BlockMsgSender, FetchMsg, FetchMsgReceiver, HookMsg, ProvingMsg};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{select, spawn, sync::Mutex, task::JoinHandle};
use tracing::{error, info};

// sub block fetcher for fetching the latest blocks by a count specified requested number of blocks
#[derive(Constructor)]
pub struct ProvingLatestFetcher {
    // fetcher configuration
    config: Arc<BlockFetcherConfig>,

    // receiving fetch messages
    fetch_receiver: Arc<Mutex<FetchMsgReceiver>>,

    // sending proving and hook messages to the main scheduler thread
    proving_sender: Arc<BlockMsgSender>,

    // executor for generating subblock and aggregation inputs
    subblock_executor: Arc<SubblockExecutor>,
}

impl ProvingLatestFetcher {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("proving-latest-fetcher: start");

        spawn(async move {
            // save the total remaining number of latest blocks
            let mut remaining_count = 0;

            // initialize a websocket rpc connection for receiving latest blocks
            let ws_conn = WsConnect::new(self.config.rpc_ws_url.as_str());
            let provider = ProviderBuilder::new()
                .connect_ws(ws_conn)
                .await
                .expect("proving-latest-fetcher: failed to connect to rpc websocket URL");
            let subscription = provider
                .subscribe_blocks()
                .await
                .expect("proving-latest-fetcher: failed to subscribe the latest blocks");
            let mut latest_block_receiver = subscription.into_stream();

            let mut fetch_receiver = self.fetch_receiver.lock().await;
            loop {
                select! {
                    // receive the latest fetch request with specified count
                    msg = fetch_receiver.recv() => if let Some(msg) = msg {
                        // handle latest block fetch message and update remaining count if necessary
                        let request_count = match msg {
                            FetchMsg::ProveLatest { count } => count,
                            msg => {
                                error!(
                                    "proving-latest-fetcher: fetch receiver received an unexpected message {msg:?}",
                                );
                                break;
                            }
                        };

                        // set the remaining count to the maximum value compared with new request
                        remaining_count = remaining_count.max(request_count);
                        info!(
                            "proving-latest-fetcher: received latest fetch message of count {request_count} and update remaining count to {remaining_count}",
                        );
                    } else {
                        info!("proving-latest-fetcher: fetch receiver is closed and will exit");
                        break;
                    },

                    block_header = latest_block_receiver.next() => if let Some(header) = block_header {
                        let block_number = header.number;
                        info!(
                            "proving-latest-fetcher: rpc websocket connection received a new block {block_number}",
                        );

                        if remaining_count > 0 {
                            info!(
                                "proving-latest-fetcher: fetching block {block_number}",
                            );
                            if let Err(e) = self.fetch_block(header).await {
                                error!(
                                    "proving-latest-fetcher: failed to fetch block {block_number} {e:?}",
                                );
                            }
                            info!("proving-latest-fetcher: succeeded for fetching block {block_number}");

                            remaining_count -= 1;
                        }
                    } else {
                        info!("proving-latest-fetcher: latest block receiver is closed and will exit");
                        break;
                    },
                }
            }
        })
    }

    // fetch a specified block by number
    async fn fetch_block(&self, header: Header) -> Result<()> {
        let block_number = header.number;
        let block_timestamp = header.timestamp;

        // send the fetch start hook message
        let msg = BlockMsg::Hook(HookMsg::FetchStart { block_number });
        self.proving_sender.send(msg)?;

        // generate proving inputs of the specified block number
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let proving_inputs = self
            .subblock_executor
            .generate_inputs(true, block_number)
            .await?;
        let end_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // create a block report
        let fetch_report = BlockProvingReport::new(
            block_number,
            block_timestamp,
            end_time - start_time,
            start_time,
            end_time,
        );

        // send the proving message
        let msg = BlockMsg::Proving(ProvingMsg::new(fetch_report, proving_inputs));
        self.proving_sender.send(msg)?;

        // send the fetch end hook message
        let msg = BlockMsg::Hook(HookMsg::FetchEnd { block_number });
        self.proving_sender.send(msg)?;

        Ok(())
    }
}
