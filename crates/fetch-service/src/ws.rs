use crate::service::FetchService;
use anyhow::Result;
use axum::{
    body::Bytes,
    extract::ws::{Message, WebSocket},
};
use common::channel::SingleUnboundedChannel;
use messages::{BlockMsg, WatchMsg};
use std::sync::Arc;
use tokio::{select, task::spawn_blocking};
use tracing::{error, info};

impl FetchService {
    // handle websocket messages
    pub async fn handle_ws(self: Arc<Self>, mut socket: WebSocket) -> Result<()> {
        // register a block proving monitor to receive block reports
        let channel = SingleUnboundedChannel::default();
        let msg = BlockMsg::Watch(WatchMsg::new(channel.sender()));
        self.comm_sender.send(msg)?;
        let proved_receiver = channel.receiver();

        // send a connection welcome message
        socket
            .send(Message::Text("fetch-service: client connected".into()))
            .await?;

        loop {
            select! {
                biased;

                // handle block proved message and return the proving report from websocket
                // TODO: fix to asynchronous channel for avoiding `spawn_blocking`
                msg = spawn_blocking({
                    let receiver = proved_receiver.clone();
                    move || receiver.recv()
                }) => {
                    match msg {
                        Ok(Ok(BlockMsg::Report(report))) => {
                            let report_bytes = bincode::serialize(&report)?;
                            socket.send(Message::Binary(report_bytes.into())).await?;
                        }
                        _ => {
                            error!("fetch-service: communication endpoint receives an unexpected message {msg:?}");
                            break;
                        }
                    }
                }

                // handle basic websocket messages
                msg = socket.recv() => {
                    match msg {
                        Some(Ok(msg)) => match msg {
                            Message::Ping(_) => {
                                let _ = socket.send(Message::Pong(Bytes::new())).await;
                            }
                            Message::Close(frame) => {
                                let _ = socket.send(Message::Close(frame)).await;
                                break;
                            }
                            _ => info!("fetch-service: websocket received trivial message: {msg:?}"),
                        }
                        Some(Err(e)) => {
                            error!("fetch-service: websocket error: {e}");
                            break;
                        }
                        None => {
                            info!("fetch-service: websocket stopped");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
