use crate::config::ProvingClientConfig;
use aggregator_proto::{ProveAggregationRequest, aggregator_client::AggregatorClient};
use common::inputs::ProvingInputs;
use derive_more::Constructor;
use messages::{BlockMsg, BlockMsgEndpoint};
use std::{collections::VecDeque, sync::Arc};
use subblock_proto::{ProveSubblockRequest, subblock_client::SubblockClient};
use tokio::{spawn, task::JoinHandle};
use tonic::{codec::CompressionEncoding, transport::Channel};
use tracing::{error, info};

#[derive(Constructor, Debug)]
pub struct ProvingClient {
    // proving client configuration
    config: ProvingClientConfig,

    // communication endpoint for coordinating with the main scheduler
    comm_endpoint: Arc<BlockMsgEndpoint>,
}

impl ProvingClient {
    pub fn run(self: Arc<Self>) -> JoinHandle<()> {
        info!("proving-client: start");

        spawn(async move {
            info!("proving-client: initialize aggregator and subblock proving clients");
            let mut agg_client = self.init_agg_proving_client().await;
            let mut subblock_clients = self.init_subblock_proving_clients().await;

            info!("proving-client: waiting for proving and proved messages");
            // variable for saving the block number proving in progress
            let mut proving_block_report = None;
            // queue for saving the pending messages when a block is proving
            let mut pending_msgs = VecDeque::new();
            while let Ok(msg) = self.comm_endpoint.recv() {
                match msg {
                    BlockMsg::Proving(proving_msg) => {
                        if proving_block_report.is_none() {
                            // send the proving inputs to aggregator and subblock grpc services
                            send_proving_inputs(
                                proving_msg.proving_inputs,
                                &mut agg_client,
                                &mut subblock_clients,
                            )
                            .await;

                            let report = proving_msg.fetch_report;
                            info!(
                                "proving-client: save block {} as the current proving block in progress",
                                report.block_number,
                            );
                            proving_block_report = Some(report);
                        } else {
                            info!(
                                "proving-client: save proving request of block {} to the pending queue",
                                proving_msg.fetch_report.block_number,
                            );
                            pending_msgs.push_back(proving_msg);
                        }
                    }
                    BlockMsg::Proved(proved_msg) => {
                        let mut report = proving_block_report.unwrap();
                        let block_number = report.block_number;
                        proving_block_report = None;
                        assert_eq!(
                            block_number, proved_msg.block_number,
                            "proving-client: the proved block is not consistent with the previous proving block",
                        );

                        // merge the proved result to the block report
                        if proved_msg.success {
                            report.on_proving_success(
                                proved_msg.cycles,
                                proved_msg.proving_milliseconds,
                                proved_msg.proof.unwrap(),
                            );
                        } else {
                            report.on_proving_failure();
                        }

                        info!("proving-client: send the report message of block {block_number}");
                        let msg = BlockMsg::Report(report);
                        self.comm_endpoint
                            .send(msg)
                            .expect("proving-client: failed to send report message");

                        // process the next pending block
                        if let Some(proving_msg) = pending_msgs.pop_front() {
                            // send the proving inputs to aggregator and subblock grpc services
                            send_proving_inputs(
                                proving_msg.proving_inputs,
                                &mut agg_client,
                                &mut subblock_clients,
                            )
                            .await;

                            let report = proving_msg.fetch_report;
                            info!(
                                "proving-client: save block {} as the current proving block in progress",
                                report.block_number,
                            );
                            proving_block_report = Some(report);
                        }
                    }
                    _ => error!("proving-client: received a wrong message {msg:?}"),
                }
            }
            info!("proving-client: stopped");
        })
    }

    // initialize a aggregator proving client
    pub async fn init_agg_proving_client(&self) -> AggregatorClient<Channel> {
        let max_msg_bytes = self.config.max_msg_bytes;
        let agg_url = self.config.agg_url.clone();
        AggregatorClient::connect(agg_url.to_string())
            .await
            .expect("proving-client: failed to connect to aggregator proving {agg_url}")
            .max_encoding_message_size(max_msg_bytes)
            .max_decoding_message_size(max_msg_bytes)
            .accept_compressed(CompressionEncoding::Zstd)
            .send_compressed(CompressionEncoding::Zstd)
    }

    // initialize subblock proving clients
    pub async fn init_subblock_proving_clients(&self) -> Vec<SubblockClient<Channel>> {
        let max_msg_bytes = self.config.max_msg_bytes;
        let subblock_urls = &self.config.subblock_urls;
        let mut subblock_clients = Vec::with_capacity(subblock_urls.len());
        for url in subblock_urls {
            let client = SubblockClient::connect(url.to_string())
                .await
                .expect("proving-client: failed to connect to subblock proving {url}")
                .max_encoding_message_size(max_msg_bytes)
                .max_decoding_message_size(max_msg_bytes)
                .accept_compressed(CompressionEncoding::Zstd)
                .send_compressed(CompressionEncoding::Zstd);

            subblock_clients.push(client);
        }

        subblock_clients
    }
}

async fn send_proving_inputs(
    proving_inputs: ProvingInputs,
    agg_client: &mut AggregatorClient<Channel>,
    subblock_clients: &mut [SubblockClient<Channel>],
) {
    let block_number = proving_inputs.block_number;
    let num_subblocks = proving_inputs.subblock_inputs.len();
    assert!(num_subblocks > 0, "proving-client: no subblocks");
    assert!(
        num_subblocks <= subblock_clients.len(),
        "proving-client: insufficient subblock proving services",
    );
    let num_subblocks = num_subblocks as u32;
    let subblock_public_values = bincode::serialize(&proving_inputs.subblock_public_values)
        .expect("proving-client: failed to serialize subblock public values");

    // TODO: check if this could be changed to run futures in parallel
    info!("proving-client: requesting with the aggregator input of block {block_number}");
    let req = ProveAggregationRequest {
        block_number,
        num_subblocks,
        subblock_public_values,
        input: proving_inputs.agg_input,
    };
    agg_client
        .prove_aggregation(req)
        .await
        .expect("proving-client: failed to request with the aggregator input");

    for (i, input) in proving_inputs.subblock_inputs.into_iter().enumerate() {
        info!("proving-client: requesting with the {i}-th subblock input of block {block_number}");
        let req = ProveSubblockRequest {
            block_number,
            num_subblocks,
            subblock_index: i as u32,
            input,
        };
        subblock_clients[i]
            .prove_subblock(req)
            .await
            .expect("proving-client: failed to request with the subblock input");
    }
}
