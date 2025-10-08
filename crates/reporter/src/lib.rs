use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgReceiver, WatchMsg};
use std::sync::Arc;
use tokio::task::{JoinHandle, spawn_blocking};
use tracing::{error, info};

#[derive(Constructor, Debug)]
pub struct BlockReporter {
    // communication receiver for coordinating with the main scheduler
    pub comm_receiver: Arc<BlockMsgReceiver>,
}

impl BlockReporter {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("reporter: start");

        spawn_blocking(move || {
            // saving the websocket watchers and will be removed as close if notification failed
            let mut watchers = vec![];
            while let Ok(msg) = self.comm_receiver.recv() {
                match &msg {
                    BlockMsg::Watch(WatchMsg { sender }) => {
                        watchers.push(sender.clone());
                        info!(
                            "reporter: added a new websocket watcher, the current watcher number is {}",
                            watchers.len(),
                        );
                    }
                    BlockMsg::Report(report) => {
                        let block_number = report.block_number;
                        watchers.retain(|watcher| watcher.send(msg.clone()).is_ok());
                        info!(
                            "reporter: notified the proved block {block_number} to watcher number {}",
                            watchers.len(),
                        );
                    }
                    _ => error!("proving-client: received a wrong message {msg:?}"),
                }
            }
            info!("reporter: stopped");
        })
    }
}
