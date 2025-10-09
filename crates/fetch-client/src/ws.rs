use anyhow::Result;
use common::report::BlockProvingReport;
use futures::{SinkExt, StreamExt};
use reqwest::Url;
use std::path::PathBuf;
use tokio::{
    spawn,
    time::{Duration, sleep},
};
use tracing::{error, info};
use tungstenite::{Bytes, protocol::Message};

// interval seconds for sending a websocket ping message
const WS_PING_INTERVAL: u64 = 15;

// wait proving complete for the specified number of requested blocks on a websocket connection
pub async fn wait_for_proving_complete(
    ws_url: &Url,
    mut block_count: usize,
    report_path: &Option<PathBuf>,
) -> Result<()> {
    let url = ws_url.as_str();
    info!("connecting to websocket: url = {url}");

    let (stream, resp) = tokio_tungstenite::connect_async(url).await?;
    info!("websocket connected with status: {}", resp.status());

    let (mut write, mut read) = stream.split();

    // send ping messages at intervals to keep the websocket connection alive
    let ping_thread = spawn(async move {
        let interval = Duration::from_secs(WS_PING_INTERVAL);
        let msg = Message::Ping(Bytes::new());

        loop {
            if let Err(e) = write.send(msg.clone()).await {
                error!("failed to send ping message to websocket: {e}");
                break;
            }

            sleep(interval).await
        }
    });

    // wait for receiving the proving reports of requested number of blocks
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Binary(data) => {
                // decode the returned block proving report
                let report: BlockProvingReport = bincode::deserialize(&data)?;

                if let Some(csv_file_path) = report_path {
                    // append the proving result to the csv file
                    report.append_to_csv(csv_file_path)?;
                } else {
                    // output the proving result if the csv file is not specified
                    info!("received proving result: {report}");
                }

                // for simplicity we only check the returned number
                if block_count <= 1 {
                    break;
                }
                block_count -= 1;
            }
            Message::Close(frame) => {
                info!("websocket closed by server: {frame:?}");
                break;
            }
            msg => info!("received other message from websocket: {msg:?}"),
        }
    }

    ping_thread.abort();
    info!("websocket disconnected");

    Ok(())
}
