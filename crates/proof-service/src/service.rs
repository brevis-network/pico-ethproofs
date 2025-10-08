use derive_more::Constructor;
use messages::BlockMsgSender;
use proof_proto::CompleteProvingRequest;
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};
use tracing::info;

#[derive(Constructor, Debug)]
pub struct ProofService {
    // communication sender for coordinating with the main scheduler
    pub comm_sender: Arc<BlockMsgSender>,
}

impl ProofService {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("proof-service: start");

        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

            self.comm_sender
                .send(messages::BlockMsg::Proved(CompleteProvingRequest::default()))
                .unwrap();

            info!("proof-service: stopped");
        })
    }
}
