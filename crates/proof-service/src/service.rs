use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgSender};
use proof_proto::{
    CompleteProvingRequest,
    proof_server::{Proof, ProofServer},
};
use std::sync::Arc;
use tokio::{signal::ctrl_c, spawn, task::JoinHandle};
use tonic::{
    Request, Response, Status, async_trait, codec::CompressionEncoding, service::LayerExt,
    transport::Server,
};
use tonic_web::GrpcWebLayer;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::config::ProofServiceConfig;

#[derive(Constructor, Debug)]
pub struct ProofService {
    // proof service configuration
    pub config: ProofServiceConfig,

    // communication sender for coordinating with the main scheduler
    pub comm_sender: Arc<BlockMsgSender>,
}

impl ProofService {
    pub fn run(self) -> JoinHandle<()> {
        info!("proof-service: start");

        spawn(async move {
            let addr = self.config.addr;
            let max_msg_bytes = self.config.max_msg_bytes;

            // create the base grpc service
            let grpc = ProofServer::new(self)
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
                .serve_with_shutdown(addr, async {
                    ctrl_c()
                        .await
                        .expect("proof-service: failed to wait for graceful shutdown");
                })
                .await
                .expect("proof-service: failed to start service");

            info!("proof-service: stopped");
        })
    }
}

#[async_trait]
impl Proof for ProofService {
    async fn complete_proving(
        &self,
        request: Request<CompleteProvingRequest>,
    ) -> Result<Response<()>, Status> {
        // send the proved message
        let proved_msg = request.into_inner();
        let block_number = proved_msg.block_number;
        info!("proof-service: received the proof result of block {block_number}");
        let msg = BlockMsg::Proved(proved_msg);
        self.comm_sender
            .send(msg)
            .expect("proof-service: failed to send a proved message of block {block_number}");

        Ok(Response::new(()))
    }
}
