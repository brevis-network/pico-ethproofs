use crate::config::BlockFetcherConfig;
use anyhow::Result;
use common::{inputs::ProvingInputs, report::BlockProvingReport};
use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgSender, FetchMsg, FetchMsgReceiver, ProvingMsg};
use std::{sync::Arc, time::Instant};
use tokio::{spawn, sync::Mutex, task::JoinHandle};
use tracing::{error, info};

// sub block fetcher for reproducing blocks by a start block number and a count specified requested
// number of blocks
#[derive(Constructor)]
pub struct ReproducingFromStartFetcher {
    // fetcher configuration
    config: Arc<BlockFetcherConfig>,

    // receiving fetch messages
    fetch_receiver: Arc<Mutex<FetchMsgReceiver>>,

    // sending proving messages to the proving-client thread
    proving_sender: Arc<BlockMsgSender>,
}

impl ReproducingFromStartFetcher {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("reproducing-from-start-fetcher: start");

        spawn(async move {
            let mut fetch_receiver = self.fetch_receiver.lock().await;
            while let Some(msg) = fetch_receiver.recv().await {
                match msg {
                    FetchMsg::ReproduceFromStart {
                        start_block_number,
                        count,
                    } => {
                        info!(
                            "reproducing-from-start-fetcher: received from-start fetch message of start_block_number = {start_block_number}, count = {count}",
                        );
                        for block_number in start_block_number..start_block_number + count {
                            info!(
                                "reproducing-from-start-fetcher: starting for fetching block {block_number}"
                            );
                            match self.load_block(block_number) {
                                Ok(()) => info!(
                                    "reproducing-from-start-fetcher: succeeded for fetching block {block_number}",
                                ),
                                Err(e) => error!(
                                    "reproducing-from-start-fetcher: failed to fetch block-{block_number} {e:?}",
                                ),
                            }
                        }
                    }
                    _ => error!("reproducing-from-start-fetcher: received a wrong message {msg:?}"),
                }
            }
        })
    }

    // load a specified block by number
    fn load_block(&self, block_number: u64) -> Result<()> {
        // generate proving inputs of the specified block number
        let input_load_dir = self
            .config
            .input_load_dir
            .as_ref()
            .expect("reproducing-from-start-fetcher: `input_load_dir` in unset");
        let start_time = Instant::now();
        let proving_inputs = ProvingInputs::load_from_dir(block_number, input_load_dir)?;
        let data_fetch_milliseconds = start_time.elapsed().as_millis() as u64;

        // create a block report
        let fetch_report = BlockProvingReport::new(block_number, data_fetch_milliseconds);

        // send the proving message
        let msg = BlockMsg::Proving(ProvingMsg::new(fetch_report, proving_inputs));
        self.proving_sender.send(msg)?;

        Ok(())
    }
}
