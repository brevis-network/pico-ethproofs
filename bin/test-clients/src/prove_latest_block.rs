use anyhow::Result;
use clap::Parser;
use common::{fetch::ProveLatestBlockParams, logger::setup_logger};
use dotenvy::dotenv;
use fetch_client::{http::prove_latest_block, ws::wait_for_proving_complete};
use reqwest::Url;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[clap(long, default_value = "1", help = "Number of requested latest blocks")]
    pub count: u64,

    #[clap(
        long,
        default_value = "proving_report.csv",
        help = "CSV file path containing the proving result"
    )]
    pub report_path: PathBuf,

    #[clap(
        long,
        env = "FETCH_HTTP_URL",
        default_value = "http://127.0.0.1:8080",
        help = "Fetch service HTTP URL"
    )]
    pub http_url: Url,

    #[clap(
        long,
        env = "FETCH_WS_URL",
        default_value = "ws://127.0.0.1:8080",
        help = "Fetch service websocket URL"
    )]
    pub ws_url: Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    // setup env and logger
    dotenv().ok();
    setup_logger();

    // parse the cli arguments
    let args = Args::parse();

    // send a http request for proving latest blocks
    let params = ProveLatestBlockParams::new(Some(args.count));
    prove_latest_block(&args.http_url, &params).await?;

    // wait for the proving result by a websocket connection
    wait_for_proving_complete(&args.ws_url, args.count as usize, &Some(args.report_path)).await
}
