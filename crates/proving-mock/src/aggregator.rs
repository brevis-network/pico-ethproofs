use crate::{
    config::{
        MOCK_CYCLES, MOCK_PROOF, MOCK_PROVING_AGGREGATOR_ADDR, MOCK_PROVING_MILLISECONDS,
        MockProvingServiceConfig,
    },
    service::MockProvingService,
};
use aggregator_proto::{
    ProveAggregationRequest,
    aggregator_server::{Aggregator, AggregatorServer},
};
use derive_more::Constructor;
use proof_proto::{CompleteProvingRequest, proof_client::ProofClient};
use std::{net::SocketAddr, sync::Arc};
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
    // return the mock aggregator grpc address
    pub fn aggregator_addr(&self) -> SocketAddr {
        MOCK_PROVING_AGGREGATOR_ADDR
            .parse()
            .expect("mock-proving-agg-service: failed to parse aggregator address")
    }

    // start the mock aggregator grpc service
    pub fn run_aggregator_service(self: Arc<Self>) -> JoinHandle<()> {
        info!("mock-proving-agg-service: start mock aggregator grpc service");

        spawn(async move {
            let max_msg_bytes = self.config.max_msg_bytes;

            // create the base grpc service
            let mock_service = MockAggregatorService::new(self.config.clone());
            let grpc = AggregatorServer::new(mock_service)
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
                .serve_with_shutdown(self.aggregator_addr(), async {
                    ctrl_c()
                        .await
                        .expect("mock-proving-agg-service: failed to wait for graceful shutdown");
                })
                .await
                .expect("mock-proving-agg-service: failed to start service");

            info!("mock-proving-agg-service: mock aggregator grpc service stopped");
        })
    }
}

// mock aggregator grpc service
#[derive(Constructor, Debug)]
struct MockAggregatorService {
    config: Arc<MockProvingServiceConfig>,
}

#[async_trait]
impl Aggregator for MockAggregatorService {
    async fn prove_aggregation(
        &self,
        request: Request<ProveAggregationRequest>,
    ) -> Result<Response<()>, Status> {
        // get the request block number
        let request = request.into_inner();
        let block_number = request.block_number;
        info!(
            "mock-proving-agg-service: received aggregation proving request of block {block_number}",
        );

        // create a proof return grpc client
        let max_msg_bytes = self.config.max_msg_bytes;
        let proof_url = self.config.proof_service_url.clone();
        let mut client = ProofClient::connect(proof_url.to_string())
            .await
            .expect("mock-proving-agg-service: failed to connect to proof return service {url}")
            .max_encoding_message_size(max_msg_bytes)
            .max_decoding_message_size(max_msg_bytes)
            .accept_compressed(CompressionEncoding::Zstd)
            .send_compressed(CompressionEncoding::Zstd);

        info!(
            "mock-proving-agg-service: requesting to return the proving result of block {block_number}",
        );
        let req = CompleteProvingRequest {
            success: true,
            block_number,
            cycles: MOCK_CYCLES,
            proving_milliseconds: MOCK_PROVING_MILLISECONDS,
            proof: Some(MOCK_PROOF.to_vec()),
        };
        client
            .complete_proving(req)
            .await
            .expect("mock-proving-agg-service: failed to request to return the proving result");

        Ok(Response::new(()))
    }
}
