use anyhow::Result;
use clap::Parser;
use common::{fetch::ProveBlockByNumberParams, logger::setup_logger};
use dotenvy::dotenv;
use fetch_client::{http::prove_block_by_number, ws::wait_for_proving_complete};
use reqwest::Url;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[clap(long, help = "Requested start block number to prove")]
    pub start_block_num: u64,

    #[clap(long, default_value = "1", help = "Number of requested blocks")]
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

    // send a http request for proving a block by the block number
    let params = ProveBlockByNumberParams::new(args.start_block_num, Some(args.count));
    prove_block_by_number(&args.http_url, &params).await?;

    // wait for the proving result by a websocket connection
    let reports = wait_for_proving_complete(&args.ws_url, args.count as usize).await?;

    // save the proving reports to a csv file
    reports
        .iter()
        .map(|r| r.append_to_csv(&args.report_path))
        .collect::<Result<Vec<_>>>()?;

    Ok(())
}
