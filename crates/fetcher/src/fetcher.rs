use crate::{
    config::BlockFetcherConfig, from_start::FromStartFetcher, latest::LatestFetcher,
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

    // sending fetch messages of `from-start` type to the specified fetcher
    from_start_msg_sender: Arc<FetchMsgSender>,

    // sending fetch messages of `latest` type to the specified fetcher
    latest_msg_sender: Arc<FetchMsgSender>,

    // fetching blocks by a start block number and a count specified the number of blocks
    from_start_fetcher: Arc<FromStartFetcher>,

    // fetching latest blocks by a count specified the number of blocks
    latest_fetcher: Arc<LatestFetcher>,
}

impl BlockFetcher {
    pub fn new(config: Arc<BlockFetcherConfig>, comm_endpoint: Arc<BlockMsgEndpoint>) -> Arc<Self> {
        // create the subblock executor
        let subblock_executor = Arc::new(SubblockExecutor::new(config.clone()));

        // create channels for communication with the sub fetchers
        let [
            (from_start_msg_sender, from_start_msg_receiver),
            (latest_msg_sender, latest_msg_receiver),
        ] = [0, 1].map(|_| {
            let channel = SingleUnboundedChannel::default();
            (channel.sender(), channel.receiver())
        });

        // initialize sub fetchers
        let from_start_fetcher = FromStartFetcher::new(
            from_start_msg_receiver,
            comm_endpoint.clone_sender(),
            subblock_executor.clone(),
        )
        .into();
        let latest_fetcher = LatestFetcher::new(
            config,
            latest_msg_receiver,
            comm_endpoint.clone_sender(),
            subblock_executor,
        )
        .into();

        Self {
            comm_endpoint,
            from_start_msg_sender,
            latest_msg_sender,
            from_start_fetcher,
            latest_fetcher,
        }
        .into()
    }

    pub fn run(self: Arc<Self>) -> Vec<JoinHandle<()>> {
        info!("fetcher: start");

        // start the sub fetcher threads
        let mut handles = vec![];
        handles.push(self.from_start_fetcher.clone().run());
        handles.push(self.latest_fetcher.clone().run());

        let comm_endpoint = self.comm_endpoint.clone();
        let from_start_msg_sender = self.from_start_msg_sender.clone();
        let latest_msg_sender = self.latest_msg_sender.clone();

        // start the main fetcher thread
        handles.push(spawn(async move {
            while let Ok(msg) = comm_endpoint.recv() {
                match msg {
                    BlockMsg::Fetch(fetch_msg) => match fetch_msg {
                        FetchMsg::FromStart { .. } => from_start_msg_sender.send(fetch_msg).expect(
                            "fetcher: failed to send a message to from-start-fetcher thread",
                        ),
                        FetchMsg::Latest { .. } => latest_msg_sender
                            .send(fetch_msg)
                            .expect("fetcher: failed to send a message to latest-fetcher thread"),
                    },
                    _ => error!("fetcher: received a wrong message {msg:?}"),
                }
            }
        }));

        handles
    }
}
