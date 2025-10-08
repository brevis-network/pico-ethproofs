use derive_more::Constructor;
use messages::BlockMsgReceiver;
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};
use tracing::info;

#[derive(Constructor, Debug)]
pub struct BlockReporter {
    // communication receiver for coordinating with the main scheduler
    pub comm_sender: Arc<BlockMsgReceiver>,
}

impl BlockReporter {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("reporter: start");

        spawn(async move { tokio::time::sleep(tokio::time::Duration::from_secs(60_000)).await })
    }
}
