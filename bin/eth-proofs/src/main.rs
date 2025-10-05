use anyhow::Result;
use clap::Parser;
use common::{
    channel::{DuplexUnboundedChannel, DuplexUnboundedEndpoint},
    logger::setup_logger,
};
use dotenvy::dotenv;
use fetch_service::{config::FetchServiceConfig, service::FetchService};
use futures::future::join_all;
use messages::BlockMsg;
use std::{net::SocketAddr, sync::Arc};

type BlockMsgEndpoint = DuplexUnboundedEndpoint<BlockMsg, BlockMsg>;

#[derive(Parser)]
struct Args {
    #[clap(
        long,
        env = "FETCH_SERVICE_ADDR",
        default_value = "[::]:8080",
        help = "Fetch service socket address"
    )]
    pub fetch_service_addr: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    // setup env and logger
    dotenv().ok();
    setup_logger();

    // parse the cli arguments
    let args = Args::parse();

    // collect the thread handles
    let mut handles = vec![];

    // initialize fetch service
    let (fetch_service, _fetch_service_endpoint) = init_fetch_service(&args);

    // start the fetch service
    handles.push(fetch_service.run());

    // wait for the all threads exit
    join_all(handles).await;

    Ok(())
}

// initialize fetch service with a communication endpoint
fn init_fetch_service(args: &Args) -> (Arc<FetchService>, Arc<BlockMsgEndpoint>) {
    // create communication channel
    let comm_channel = DuplexUnboundedChannel::default();

    // create fetch service
    let config = FetchServiceConfig::new(args.fetch_service_addr);
    let service = FetchService::new(config, comm_channel.endpoint1()).into();

    (service, comm_channel.endpoint2())
}
