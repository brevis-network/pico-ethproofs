use anyhow::Result;
use common::report::BlockProvingReport;
use futures::{SinkExt, StreamExt};
use reqwest::Url;
use std::path::PathBuf;
use tokio::{
    select, spawn,
    sync::oneshot,
    time::{Duration, sleep},
};
use tracing::{error, info};
use tungstenite::{Bytes, protocol::Message};

// interval seconds for sending a websocket ping message
const WS_PING_INTERVAL: u64 = 15;

// wait proving complete for the specified number of requested blocks on a websocket connection
// - ws_url: websocket URL to connect
// - block_count: number of blocks to wait for complete
// - report_path: csv file to append the block reports if it's specified
pub async fn wait_for_proving_complete(
    ws_url: &Url,
    mut block_count: usize,
    report_path: &Option<PathBuf>,
) -> Result<()> {
    let url = ws_url.as_str();
    info!("websocket-client: connecting to {url}");

    let (ws_stream, ws_resp) = tokio_tungstenite::connect_async(url).await?;
    info!(
        "websocket-client: connected with status {}",
        ws_resp.status(),
    );

    // split to a websocket sender and receiver
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // create a oneshot channel for graceful shutdown
    let (exit_sender, mut exit_receiver) = oneshot::channel();

    // send ping messages at intervals to keep the websocket connection alive
    let ping_thread = spawn(async move {
        let ping_interval = Duration::from_secs(WS_PING_INTERVAL);
        let ping_msg = Message::Ping(Bytes::new());

        loop {
            select! {
                _ = sleep(ping_interval) => {
                    if let Err(e) = ws_sender.send(ping_msg.clone()).await {
                        error!("websocket-client: failed to send ping message {e}");
                        break;
                    }
                }
                _ = &mut exit_receiver => {
                    info!("websocket-client: sending a Close meesage before exit");
                    let _ = ws_sender.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    });

    // wait for receiving the proving reports of requested number of blocks
    while let Some(msg) = ws_receiver.next().await {
        match msg? {
            Message::Binary(data) => {
                // decode the returned block proving report
                let report: BlockProvingReport = bincode::deserialize(&data)?;

                if let Some(csv_file_path) = report_path {
                    // append the proving result to the csv file
                    report.append_to_csv(csv_file_path)?;
                } else {
                    // output the proving result if the csv file is not specified
                    info!("websocket-client: received proving result {report}");
                }

                // for simplicity we only check the returned number
                if block_count <= 1 {
                    break;
                }
                block_count -= 1;
            }
            Message::Close(frame) => {
                info!("websocket-client: closed by server {frame:?}");
                break;
            }
            msg => info!("websocket-client: received other message {msg:?}"),
        }
    }

    // send a exit message to the websocket ping thread
    let _ = exit_sender.send(());
    let _ = ping_thread.await;

    info!("websocket-client: disconnected");

    Ok(())
}
