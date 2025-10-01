use anyhow::Result;
use common::report::BlockProvingReport;
use futures::{SinkExt, StreamExt};
use reqwest::Url;
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
    block_count: usize,
) -> Result<Vec<BlockProvingReport>> {
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
    let mut reports = Vec::with_capacity(block_count);
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Binary(data) => {
                // decode the returned block proving report
                let report = bincode::deserialize(&data)?;
                reports.push(report);

                // for simplicity we only check the returned number
                if reports.len() == block_count {
                    break;
                }
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

    Ok(reports)
}
