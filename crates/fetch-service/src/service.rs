use crate::config::FetchServiceConfig;
use axum::{
    Router,
    extract::{Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use common::fetch::{
    HTTP_PROVE_BLOCK_BY_NUMBER_PATH, HTTP_PROVE_LATEST_BLOCK_PATH, ProveBlockByNumberParams,
    ProveLatestBlockParams,
};
use derive_more::Constructor;
use messages::BlockMsgSender;
use std::sync::Arc;
use tokio::{net::TcpListener, signal::ctrl_c, spawn, task::JoinHandle};
use tracing::{error, info};

// fetch http and websocket service
#[derive(Constructor, Debug)]
pub struct FetchService {
    // fetch service configuration
    pub config: FetchServiceConfig,

    // communication sender for coordinating with the main scheduler
    pub comm_sender: Arc<BlockMsgSender>,
}

impl FetchService {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("fetch-service: start");

        let addr = self.config.addr;
        spawn(async move {
            // create the router for http and websocket service
            let router = Router::new()
                // root path is used for websocket, it notifies the proving result to client
                .route("/", get(ws_handler))
                // HTTP Get request path for proving blocks by the specified block number
                // It supports two parameters:
                // - start_block_num: it specifies the `start` block number to prove
                // - count: it's optional and `1` is the default value, it specifies the number of blocks to prove
                .route(HTTP_PROVE_BLOCK_BY_NUMBER_PATH, get(prove_block_by_number))
                // HTTP Get request path for proving latest blocks
                // It supports one parameter:
                // - count: it's optional and `1` is the default value, it specifies the number of latest blocks
                //   to prove
                .route(HTTP_PROVE_LATEST_BLOCK_PATH, get(prove_latest_block))
                .with_state(self);

            // listen on the specified socket address
            let listener = TcpListener::bind(addr)
                .await
                .expect("fetch-service: failed to listening on {addr}");
            info!("fetch-service: listening on {addr}");

            // start the service
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .expect("fetch-service: failed to start");
        })
    }
}

// handle websocket messages
async fn ws_handler(
    State(service): State<Arc<FetchService>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        if let Err(err) = service.handle_ws(socket).await {
            error!("fetch-service: websocket returns error {err}");
        }
    })
}

// handle `prove_block_by_number` HTTP Get request
async fn prove_block_by_number(
    State(service): State<Arc<FetchService>>,
    Query(params): Query<ProveBlockByNumberParams>,
) -> impl IntoResponse {
    info!("fetch-service: received prove_block_by_number with params {params:?}");

    service.prove_block_by_number(params).map_or_else(
        |e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        |_| (StatusCode::OK, "OK".to_string()),
    )
}

// handle `prove_latest_block` HTTP Get request
async fn prove_latest_block(
    State(service): State<Arc<FetchService>>,
    Query(params): Query<ProveLatestBlockParams>,
) -> impl IntoResponse {
    info!("fetch-service: received prove_latest_block with params {params:?}");

    service.prove_latest_block(params).map_or_else(
        |e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        |_| (StatusCode::OK, "OK".to_string()),
    )
}

// graceful shutdown for `Ctrl+C`
async fn shutdown_signal() {
    ctrl_c().await.expect("failed to install Ctrl+C handler");
    info!("Ctrl+C signal received");
}
