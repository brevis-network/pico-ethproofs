use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgEndpoint, BlockMsgReceiver, BlockMsgSender};
use std::sync::Arc;
use tokio::{select, spawn, sync::Mutex, task::JoinHandle};
use tracing::{error, info};

// main scheduler for coordinating multiple threads
// the main process is:
// fetch-service-http -> fetcher -> proving-client -> proving-cluster -> proof-service ->
// reporter -> fetch-service-websocket
// - fetch-service receives a http request and sends a FetchMsg to fetcher thread
// - fetcher thread gets block data via rpc node, generates and sends subblock and aggregation
//   inputs to proving-client thread, and sends fetch performance report to reporter thread
// - proving-client thread sends subblock and aggregation inputs to proving-cluster
// - after proving complete proving-cluster sends the proof result to proof-service by grpc
// - proof-service sends the proof result to reporter thread
// - reporter thread collects and calculates the final block proving report to each fetch-service
//   websocket connection, each websocket connection receives the all proving results which should
//   be filtered by the users
#[derive(Constructor)]
pub struct Scheduler {
    // receiving and handling fetch requests from fetch-service
    fetch_service_receiver: Arc<Mutex<BlockMsgReceiver>>,

    // receiving and handling proving results
    proof_service_receiver: Arc<Mutex<BlockMsgReceiver>>,

    // bidirectional endpoint for receiving the fetch requests and sending the proving requests
    fetcher_endpoint: Arc<BlockMsgEndpoint>,

    // bidirectional endpoint for receiving the proving requests and sending the block reports
    proving_client_endpoint: Arc<BlockMsgEndpoint>,

    // sending the block reports to the reporter thread
    reporter_sender: Arc<BlockMsgSender>,
}

impl Scheduler {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("scheduler: start");

        let fetch_service_receiver = self.fetch_service_receiver.clone();
        let proof_service_receiver = self.proof_service_receiver.clone();
        let fetcher_endpoint = self.fetcher_endpoint.clone();
        let proving_client_endpoint = self.proving_client_endpoint.clone();
        let report_sender = self.reporter_sender.clone();

        spawn(async move {
            let mut fetch_service_receiver = fetch_service_receiver.lock().await;
            let mut proof_service_receiver = proof_service_receiver.lock().await;
            loop {
                select! {
                    msg = fetch_service_receiver.recv() => {
                        let msg = msg.expect("scheduler: received an error message from fetch-service");
                        match msg {
                            BlockMsg::Fetch(_) => {
                                fetcher_endpoint.send(msg).expect("scheduler: failed to send a fetch message to fetcher thread");
                            }
                            BlockMsg::Watch(_) => {
                                report_sender.send(msg).expect("scheduler: failed to send a watch message to reporter thread");
                            }
                            _ => {
                                error!("scheduler: received a wrong message from fetch-service {msg:?}");
                            }
                        }
                    }
                    msg = proof_service_receiver.recv() => {
                        let msg = msg.expect("scheduler: received an error message from proof-service");
                        match msg {
                            BlockMsg::Proved(_) => {
                                proving_client_endpoint.send(msg).expect("scheduler: failed to send a proved message to proving-client thread");
                            }
                            _ => {
                                error!("scheduler: received a wrong message from proof-service {msg:?}");
                            }
                        }
                    }
                    msg = fetcher_endpoint.recv() => {
                        let msg = msg.expect("scheduler: received an error message from fetcher thread");
                        match msg {
                            BlockMsg::Proving(_) => {
                                proving_client_endpoint.send(msg).expect("scheduler: failed to send a proving message to proving-client thread");
                            }
                            _ => {
                                error!("scheduler: received a wrong message from fetcher thread {msg:?}");
                            }
                        }
                    }
                    msg = proving_client_endpoint.recv() => {
                        let msg = msg.expect("scheduler: received an error message from proving-client thread");
                        match msg {
                            BlockMsg::Report(_) => {
                                report_sender.send(msg).expect("scheduler: failed to send a report message to reporter thread");
                            }
                            _ => {
                                error!("scheduler: received a wrong message from proving-client thread {msg:?}");
                            }
                        }
                    }
                }
            }
        })
    }
}
