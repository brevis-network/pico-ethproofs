use anyhow::Result;
use clap::Parser;
use common::{
    channel::{DuplexUnboundedChannel, SingleUnboundedChannel},
    logger::setup_logger,
};
use dotenvy::dotenv;
use fetch_service::{config::FetchServiceConfig, service::FetchService};
use fetcher::{config::BlockFetcherConfig, fetcher::BlockFetcher};
use futures::future::join_all;
use messages::{BlockMsgEndpoint, BlockMsgReceiver, BlockMsgSender};
use proof_service::{config::ProofServiceConfig, service::ProofService};
use reporter::BlockReporter;
use reqwest::Url;
use scheduler::Scheduler;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

#[derive(Parser)]
struct Args {
    #[clap(
        long,
        default_value = "false",
        help = "identify if should check the generated inputs by emulation"
    )]
    is_input_emulated: bool,

    #[clap(
        long,
        help = "Base directory for saving input files; nothing will be saved if not specified"
    )]
    input_dump_dir: Option<PathBuf>,

    #[clap(long, env = "RPC_HTTP_URL", help = "RPC node HTTP URL")]
    rpc_http_url: Url,

    #[clap(long, env = "RPC_WS_URL", help = "RPC node websocket URL")]
    rpc_ws_url: Url,

    #[clap(
        long,
        env = "SUBBLOCK_ELF_PATH",
        default_value = "data/subblock-elf",
        help = "Subblock ELF file path"
    )]
    subblock_elf_path: PathBuf,

    #[clap(
        long,
        env = "AGG_ELF_PATH",
        default_value = "data/aggregator-elf",
        help = "Aggregator ELF file path"
    )]
    agg_elf_path: PathBuf,

    #[clap(
        long,
        env = "FETCH_SERVICE_ADDR",
        default_value = "[::]:8080",
        help = "Fetch service socket address"
    )]
    fetch_service_addr: SocketAddr,

    #[clap(
        long,
        env = "PROOF_SERVICE_ADDR",
        default_value = "[::]:50052",
        help = "Proof service GRPC address"
    )]
    pub proof_service_addr: SocketAddr,

    #[clap(
        long,
        env = "MAX_GRPC_MSG_BYTES",
        default_value = "1073741824",
        help = "Maximum GRPC message bytes"
    )]
    pub max_grpc_msg_bytes: usize,
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
    let (fetcher, fetcher_endpoint) = init_fetcher(&args);

    // initialize proving client thread
    let (_proving_client, proving_client_sender) = init_proving_client(&args);

    // initialize proof service
    let (proof_service, proof_service_receiver) = init_proof_service(&args);

    // initialize reporter thread
    let (reporter, reporter_sender) = init_reporter(&args);

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

    // TODO: start the proving client thread

    // start the fetcher thread
    handles.extend(fetcher.run());

    // start the fetch service
    handles.push(fetch_service.run());

    // start the reporter thread
    handles.push(reporter.run());

    // start the proof service
    handles.push(proof_service.run());

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
fn init_fetcher(args: &Args) -> (Arc<BlockFetcher>, Arc<BlockMsgEndpoint>) {
    // create communication channel
    let comm_channel = DuplexUnboundedChannel::default();

    // create fetcher instance
    let config = BlockFetcherConfig::new(
        args.is_input_emulated,
        args.input_dump_dir.clone(),
        args.rpc_http_url.clone(),
        args.rpc_ws_url.clone(),
        args.subblock_elf_path.clone(),
        args.agg_elf_path.clone(),
    )
    .into();
    let fetcher = BlockFetcher::new(config, comm_channel.endpoint1());

    (fetcher, comm_channel.endpoint2())
}

// initialize proving client thread
fn init_proving_client(_args: &Args) -> ((), Arc<BlockMsgSender>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    ((), comm_channel.sender())
}

// initialize proof service
fn init_proof_service(args: &Args) -> (ProofService, Arc<BlockMsgReceiver>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    // create proof service
    let config = ProofServiceConfig::new(args.proof_service_addr, args.max_grpc_msg_bytes);
    let service = ProofService::new(config, comm_channel.sender());

    (service, comm_channel.receiver())
}

// initialize reporter thread
fn init_reporter(_args: &Args) -> (Arc<BlockReporter>, Arc<BlockMsgSender>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    // create reporter instance
    let reporter = BlockReporter::new(comm_channel.receiver()).into();

    (reporter, comm_channel.sender())
}
