use crate::{
    config::BlockFetcherConfig, proving_from_start::ProvingFromStartFetcher,
    proving_latest::ProvingLatestFetcher, reproducing_from_start::ReproducingFromStartFetcher,
    subblock_executor::SubblockExecutor,
};
use common::channel::SingleUnboundedChannel;
use messages::{BlockMsg, BlockMsgEndpoint, FetchMsg, FetchMsgSender};
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};
use tracing::{error, info};

// main block fetcher for dispatching different types of fetch messages
pub struct BlockFetcher {
    // communication endpoint for coordinating with the main scheduler
    comm_endpoint: Arc<BlockMsgEndpoint>,

    // sending fetch messages of `prove-from-start` type to the specified fetcher
    proving_from_start_msg_sender: Arc<FetchMsgSender>,

    // sending fetch messages of `prove-latest` type to the specified fetcher
    proving_latest_msg_sender: Arc<FetchMsgSender>,

    // sending fetch messages of `reproduce-from-start` type to the specified fetcher
    reproducing_from_start_msg_sender: Arc<FetchMsgSender>,

    // fetching blocks by a start block number and a count specified the number of blocks
    proving_from_start_fetcher: Arc<ProvingFromStartFetcher>,

    // fetching latest blocks by a count specified the number of blocks
    proving_latest_fetcher: Arc<ProvingLatestFetcher>,

    // reproducing blocks by a start block number and a count specified the number of blocks
    reproducing_from_start_fetcher: Arc<ReproducingFromStartFetcher>,
}

impl BlockFetcher {
    pub fn new(config: Arc<BlockFetcherConfig>, comm_endpoint: Arc<BlockMsgEndpoint>) -> Arc<Self> {
        // create the subblock executor
        let subblock_executor = Arc::new(SubblockExecutor::new(config.clone()));

        // create channels for communication with the sub fetchers
        let [
            (proving_from_start_msg_sender, proving_from_start_msg_receiver),
            (proving_latest_msg_sender, proving_latest_msg_receiver),
            (reproducing_from_start_msg_sender, reproducing_from_start_msg_receiver),
        ] = [0, 1, 2].map(|_| {
            let channel = SingleUnboundedChannel::default();
            (channel.sender(), channel.receiver())
        });

        // initialize sub fetchers
        let proving_from_start_fetcher = ProvingFromStartFetcher::new(
            proving_from_start_msg_receiver,
            comm_endpoint.clone_sender(),
            subblock_executor.clone(),
        )
        .into();
        let proving_latest_fetcher = ProvingLatestFetcher::new(
            config.clone(),
            proving_latest_msg_receiver,
            comm_endpoint.clone_sender(),
            subblock_executor,
        )
        .into();
        let reproducing_from_start_fetcher = ReproducingFromStartFetcher::new(
            config,
            reproducing_from_start_msg_receiver,
            comm_endpoint.clone_sender(),
        )
        .into();

        Self {
            comm_endpoint,
            proving_from_start_msg_sender,
            proving_latest_msg_sender,
            reproducing_from_start_msg_sender,
            proving_from_start_fetcher,
            proving_latest_fetcher,
            reproducing_from_start_fetcher,
        }
        .into()
    }

    pub fn run(self: Arc<Self>) -> Vec<JoinHandle<()>> {
        info!("fetcher: start");

        // start the sub fetcher threads
        let mut handles = vec![];
        handles.push(self.proving_from_start_fetcher.clone().run());
        handles.push(self.proving_latest_fetcher.clone().run());
        handles.push(self.reproducing_from_start_fetcher.clone().run());

        let comm_endpoint = self.comm_endpoint.clone();
        let proving_from_start_msg_sender = self.proving_from_start_msg_sender.clone();
        let proving_latest_msg_sender = self.proving_latest_msg_sender.clone();
        let reproducing_from_start_msg_sender = self.reproducing_from_start_msg_sender.clone();

        // start the main fetcher thread
        handles.push(spawn(async move {
            while let Ok(msg) = comm_endpoint.recv() {
                match msg {
                    BlockMsg::Fetch(fetch_msg) => match fetch_msg {
                        FetchMsg::ProveFromStart { .. } => {
                            proving_from_start_msg_sender.send(fetch_msg).expect(
                                "fetcher: failed to send a message to proving-from-start-fetcher thread",
                            )
                        }
                        FetchMsg::ProveLatest { .. } => proving_latest_msg_sender
                            .send(fetch_msg)
                            .expect("fetcher: failed to send a message to proving-latest-fetcher thread"),
                        FetchMsg::ReproduceFromStart { .. } => {
                            reproducing_from_start_msg_sender.send(fetch_msg).expect(
                                "fetcher: failed to send a message to reproducing-from-start-fetcher thread",
                            )
                        }
                    },
                    _ => error!("fetcher: received a wrong message {msg:?}"),
                }
            }
        }));

        handles
    }
}
