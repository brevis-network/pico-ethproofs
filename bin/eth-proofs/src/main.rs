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
use proving_client::{client::ProvingClient, config::ProvingClientConfig};
use proving_mock::{config::MockProvingServiceConfig, service::MockProvingService};
use reporter::BlockReporter;
use reqwest::Url;
use scheduler::Scheduler;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Parser)]
struct Args {
    #[clap(
        long,
        default_value = "false",
        help = "identify if enable mock proving service (only used for testing)"
    )]
    is_mock_proving: bool,

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

    #[clap(
        long,
        help = "Base directory for reproducing blocks by loading input files; it could be the same directory as `input_dump_dir`"
    )]
    input_load_dir: Option<PathBuf>,

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

    #[clap(
        long,
        env = "PROVING_AGG_URL",
        help = "Aggregator proving GRPC URL to request"
    )]
    pub proving_agg_url: Option<Url>,

    #[clap(
        long,
        env = "PROVING_SUBBLOCK_URLS",
        value_delimiter = ',',
        help = "Subbblock proving GRPC URLs separated by comma, e.g. `http://172.1.1.1:50052,http://172.2.2.2:50052`"
    )]
    pub proving_subblock_urls: Option<Vec<Url>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // setup env and logger
    dotenv().ok();
    setup_logger();

    // parse the cli arguments
    let mut args = Args::parse();

    // collect the thread handles
    let mut handles = vec![];

    if args.is_mock_proving {
        // start mock proving service for testing and change the proving service URLs in internal
        let mock_proving_service = init_mock_proving_service(&mut args);
        handles.extend(mock_proving_service.run());
    }

    // initialize fetch service
    let (fetch_service, fetch_service_receiver) = init_fetch_service(&args);

    // initialize proof service
    let (proof_service, proof_service_receiver) = init_proof_service(&args);

    // initialize fetcher implementation thread
    let (fetcher, fetcher_endpoint) = init_fetcher(&args);

    // initialize proving client thread
    let (proving_client, proving_client_endpoint) = init_proving_client(&args);

    // initialize reporter thread
    let (reporter, reporter_sender) = init_reporter(&args);

    // initialize main scheduler
    let scheduler = Arc::new(Scheduler::new(
        fetch_service_receiver,
        proof_service_receiver,
        fetcher_endpoint,
        proving_client_endpoint,
        reporter_sender,
    ));

    // start scheduler
    handles.push(scheduler.run());

    // start the reporter thread
    handles.push(reporter.run());

    // start the proving-client thread
    handles.push(proving_client.run());

    // start the fetcher thread
    handles.extend(fetcher.run());

    // start the proof-service
    handles.push(proof_service.run());

    // start the fetch-service
    handles.push(fetch_service.run());

    // wait for the all threads exit
    join_all(handles).await;

    Ok(())
}

// initialize mock proving service
fn init_mock_proving_service(args: &mut Args) -> Arc<MockProvingService> {
    // create mock proving service
    let config = MockProvingServiceConfig::new(args.max_grpc_msg_bytes, &args.proof_service_addr);
    let service = MockProvingService::new(config);

    // reset the mock proving urls to the arguments
    args.proving_agg_url = Some(service.aggregator_url());
    args.proving_subblock_urls = Some(service.subblock_urls());

    service.into()
}

// initialize fetch-service
fn init_fetch_service(args: &Args) -> (Arc<FetchService>, Arc<Mutex<BlockMsgReceiver>>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    // create fetch service
    let config = FetchServiceConfig::new(args.fetch_service_addr);
    let service = FetchService::new(config, comm_channel.sender()).into();

    (service, comm_channel.receiver())
}

// initialize proof-service
fn init_proof_service(args: &Args) -> (ProofService, Arc<Mutex<BlockMsgReceiver>>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    // create proof service
    let config = ProofServiceConfig::new(args.proof_service_addr, args.max_grpc_msg_bytes);
    let service = ProofService::new(config, comm_channel.sender());

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
        args.input_load_dir.clone(),
        args.rpc_http_url.clone(),
        args.rpc_ws_url.clone(),
        args.subblock_elf_path.clone(),
        args.agg_elf_path.clone(),
    )
    .into();
    let fetcher = BlockFetcher::new(config, comm_channel.endpoint1());

    (fetcher, comm_channel.endpoint2())
}

// initialize proving-client thread
fn init_proving_client(args: &Args) -> (Arc<ProvingClient>, Arc<BlockMsgEndpoint>) {
    // create communication channel
    let comm_channel = DuplexUnboundedChannel::default();

    // create proving-client instance
    let config = ProvingClientConfig::new(
        args.max_grpc_msg_bytes,
        args.proving_agg_url
            .clone()
            .expect("eth-proofs: must set `proving_agg_url` or enable `is_mock_proving`"),
        args.proving_subblock_urls
            .clone()
            .expect("eth-proofs: must set `proving_subblock_urls` or enable `is_mock_proving`"),
    );
    let proving_client = ProvingClient::new(config, comm_channel.endpoint1()).into();

    (proving_client, comm_channel.endpoint2())
}

// initialize reporter thread
fn init_reporter(_args: &Args) -> (Arc<BlockReporter>, Arc<BlockMsgSender>) {
    // create communication channel
    let comm_channel = SingleUnboundedChannel::default();

    // create reporter instance
    let reporter = BlockReporter::new(comm_channel.receiver()).into();

    (reporter, comm_channel.sender())
}
