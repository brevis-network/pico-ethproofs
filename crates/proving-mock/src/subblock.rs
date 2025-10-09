use crate::{config::MOCK_PROVING_SUBBLOCK_ADDR, service::MockProvingService};
use derive_more::Constructor;
use std::{net::SocketAddr, sync::Arc};
use subblock_proto::{
    ProveSubblockRequest,
    subblock_server::{Subblock, SubblockServer},
};
use tokio::{signal::ctrl_c, spawn, task::JoinHandle};
use tonic::{
    Request, Response, Status, async_trait, codec::CompressionEncoding, service::LayerExt,
    transport::Server,
};
use tonic_web::GrpcWebLayer;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

impl MockProvingService {
    // return the mcok subblock grpc address
    pub fn subblock_addr(&self) -> SocketAddr {
        MOCK_PROVING_SUBBLOCK_ADDR
            .parse()
            .expect("mock-proving-subblock-service: failed to parse subblock address")
    }

    // start the mock subblock grpc service
    pub fn run_subblock_service(self: Arc<Self>) -> JoinHandle<()> {
        info!("mock-proving-subblock-service: start mock subblock grpc service");

        spawn(async move {
            let max_msg_bytes = self.config.max_msg_bytes;

            // create the base grpc service
            let grpc = SubblockServer::new(MockSubblockService)
                .max_encoding_message_size(max_msg_bytes)
                .max_decoding_message_size(max_msg_bytes)
                .accept_compressed(CompressionEncoding::Zstd)
                .send_compressed(CompressionEncoding::Zstd);

            // add a web layer to the grpc service
            let service = ServiceBuilder::new()
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any),
                )
                .layer(GrpcWebLayer::new())
                .into_inner()
                .named_layer(grpc);

            Server::builder()
                .accept_http1(true)
                .add_service(service)
                .serve_with_shutdown(self.subblock_addr(), async {
                    ctrl_c().await.expect(
                        "mock-proving-subblock-service: failed to wait for graceful shutdown",
                    );
                })
                .await
                .expect("mock-proving-subblock-service: failed to start service");

            info!("mock-proving-subblock-service: mock subblock grpc service stopped");
        })
    }
}

// mock subblock grpc service
#[derive(Constructor, Debug)]
struct MockSubblockService;

#[async_trait]
impl Subblock for MockSubblockService {
    async fn prove_subblock(
        &self,
        request: Request<ProveSubblockRequest>,
    ) -> Result<Response<()>, Status> {
        let request = request.into_inner();
        info!(
            "mock-proving-subblock-service: received subblock proving request of block {}, num_subblocks {}, subblock_index {}",
            request.block_number, request.num_subblocks, request.subblock_index,
        );

        Ok(Response::new(()))
    }
}
