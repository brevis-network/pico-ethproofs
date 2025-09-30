use anyhow::Result;
use common::logger::setup_logger;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_logger();

    Ok(())
}
