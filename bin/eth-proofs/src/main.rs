use anyhow::Result;
use clap::Parser;
use common::{
    channel::{DuplexUnboundedChannel, SingleUnboundedChannel},
    logger::setup_logger,
};
use dotenvy::dotenv;
use fetch_service::{config::FetchServiceConfig, service::FetchService};
use futures::future::join_all;
use messages::{BlockMsgEndpoint, BlockMsgReceiver, BlockMsgSender};
use scheduler::Scheduler;
use std::{net::SocketAddr, sync::Arc};

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
    let (fetch_service, fetch_service_receiver) = init_fetch_service(&args);

    // initialize fetcher implementation thread
    let (_fetcher, fetcher_endpoint) = init_fetcher(&args);

    // initialize proving client thread
    let (_proving_client, proving_client_sender) = init_proving_client(&args);

    // initialize proof service
    let (_proof_service, proof_service_receiver) = init_proof_service(&args);

    // initialize reporter thread
    let (_reporter, reporter_sender) = init_reporter(&args);

    // initialize main scheduler
    let scheduler = Arc::new(Scheduler::new(
        fetch_service_receiver,
        fetcher_endpoint,
        proving_client_sender,
        proof_service_receiver,
        reporter_sender,
    ));

    // start scheduler
    handles.push(scheduler.run());

    // start the fetch service
    handles.push(fetch_service.run());

    // wait for the all threads exit
    join_all(handles).await;

    Ok(())
}

// initialize fetch service
fn init_fetch_service(args: &Args) -> (Arc<FetchService>, Arc<BlockMsgReceiver>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    // create fetch service
    let config = FetchServiceConfig::new(args.fetch_service_addr);
    let service = FetchService::new(config, comm_channel.sender()).into();

    (service, comm_channel.receiver())
}

// initialize fetcher implementation thread
fn init_fetcher(_args: &Args) -> ((), Arc<BlockMsgEndpoint>) {
    // create communication channel
    let comm_channel = DuplexUnboundedChannel::default();

    ((), comm_channel.endpoint2())
}

// initialize proving client thread
fn init_proving_client(_args: &Args) -> ((), Arc<BlockMsgSender>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    ((), comm_channel.sender())
}

// initialize proof service
fn init_proof_service(_args: &Args) -> ((), Arc<BlockMsgReceiver>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    ((), comm_channel.receiver())
}

// initialize reporter thread
fn init_reporter(_args: &Args) -> ((), Arc<BlockMsgSender>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    ((), comm_channel.sender())
}
