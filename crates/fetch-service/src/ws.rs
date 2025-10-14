use crate::service::FetchService;
use anyhow::Result;
use axum::{
    body::Bytes,
    extract::ws::{Message, WebSocket},
};
use common::channel::SingleUnboundedChannel;
use futures_util::{sink::SinkExt, stream::StreamExt};
use messages::{BlockMsg, WatchMsg};
use std::sync::Arc;
use tokio::{spawn, sync::mpsc::unbounded_channel};
use tracing::{info, warn};

impl FetchService {
    // handle websocket messages
    pub async fn handle_ws(self: Arc<Self>, socket: WebSocket) -> Result<()> {
        info!("fetch-service: received a websocket in handle_ws");

        // split to a websocket sender and receiver
        let (mut ws_sender, mut ws_receiver) = socket.split();

        info!("fetch-service: registering a block proving monitor to receive block reports");
        let proved_receiver = {
            let channel = SingleUnboundedChannel::default();
            let msg = BlockMsg::Watch(WatchMsg::new(channel.sender()));
            self.comm_sender.send(msg)?;

            channel.receiver()
        };

        info!("fetch-service: sending a websocket welcome message");
        ws_sender
            .send(Message::Text(
                "fetch-service: websocket client connected".into(),
            ))
            .await?;

        // create a channel for transfering the websocket messages in different threads
        let (msg_sender, mut msg_receiver) = unbounded_channel();

        let msg_sender_clone = msg_sender.clone();
        let proved_receiving_handle = spawn(async move {
            let mut proved_receiver = proved_receiver.lock().await;
            while let Some(BlockMsg::Report(report)) = proved_receiver.recv().await {
                // serialize block report
                let report_bytes = bincode::serialize(&report)
                    .expect("fetch-service: failed to serialize block report in websocket");

                // send serialized block report to websocket sender thread
                if msg_sender_clone
                    .send(Message::Binary(report_bytes.into()))
                    .is_err()
                {
                    warn!("fetch-service: websocket connection may be closed");
                    break;
                }
            }
        });

        let ws_sending_handle = spawn(async move {
            while let Some(ws_msg) = msg_receiver.recv().await {
                ws_sender
                    .send(ws_msg)
                    .await
                    .expect("fetch-service: failed to send block report to websocket client");
            }
        });

        info!("fetch-service: handling the websocket messages from client");
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Ping(_) => {
                    info!(
                        "fetch-service: received a websocket Ping meesage and returning a Pong message",
                    );
                    let _ = msg_sender.send(Message::Pong(Bytes::new()));
                }
                Message::Close(_) => {
                    info!("fetch-service: received a websocket Close meesage and will exit");
                    break;
                }
                _ => info!("fetch-service: received trivial websocket message {msg:?}"),
            }
        }

        info!("fetch-service: closing the related threads in websocket");
        proved_receiving_handle.abort();
        ws_sending_handle.abort();
        info!("fetch-service: websocket disconnected");

        Ok(())
    }
}
