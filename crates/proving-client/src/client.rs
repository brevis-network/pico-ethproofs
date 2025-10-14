use crate::config::ProvingClientConfig;
use aggregator_proto::{ProveAggregationRequest, aggregator_client::AggregatorClient};
use common::inputs::ProvingInputs;
use derive_more::Constructor;
use itertools::Itertools;
use messages::{BlockMsg, BlockMsgEndpoint};
use std::{collections::VecDeque, sync::Arc};
use subblock_proto::{ProveSubblockRequest, subblock_client::SubblockClient};
use tokio::{
    process::Command,
    select, spawn,
    task::JoinHandle,
    time::{Duration, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tonic::{codec::CompressionEncoding, transport::Channel};
use tracing::{error, info, warn};

// maximum waiting time for proving complete
const MAX_PROVING_WAITING_SECONDS: u64 = 120;

// wait time after docker retry before reinitializing clients (in seconds)
const DOCKER_RETRY_WAIT_SECONDS: u64 = 10;

// retry interval for client connection attempts (in seconds)
const CLIENT_RETRY_INTERVAL_SECONDS: u64 = 2;

// maximum number of retries for sending proving requests
const MAX_PROVING_REQUEST_RETRIES: u32 = 50;

// retry interval for proving request attempts (in seconds)
const PROVING_REQUEST_RETRY_INTERVAL_SECONDS: u64 = 10;

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

        let cancellation_token = CancellationToken::new();
        let token = cancellation_token.clone();

        // Set up signal handling for graceful shutdown
        let shutdown_token = token.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl+c");
            info!("proving-client: received ctrl+c, initiating graceful shutdown");
            shutdown_token.cancel();
        });

        spawn(async move {
            info!("proving-client: initialize aggregator and subblock proving clients");
            let mut agg_client = self.init_agg_proving_client(&token).await;
            let mut subblock_clients = self.init_subblock_proving_clients(&token).await;

            info!("proving-client: waiting for proving and proved messages");
            // variable for saving the block number proving in progress
            let mut proving_block_report = None;
            // variable for saving the last proving inputs (for retry on timeout)
            let mut last_proving_inputs: Option<ProvingInputs> = None;
            // queue for saving the pending messages when a block is proving
            let mut pending_msgs = VecDeque::new();
            loop {
                // try to receive a proving or proved message with a timeout
                let msg = timeout(
                    Duration::from_secs(MAX_PROVING_WAITING_SECONDS),
                    self.comm_endpoint.recv(),
                )
                .await;

                match msg {
                    Ok(Ok(BlockMsg::Proving(proving_msg))) => {
                        if proving_block_report.is_none() {
                            // send the proving inputs to aggregator and subblock grpc services
                            send_proving_inputs(
                                proving_msg.proving_inputs.clone(),
                                &mut agg_client,
                                &mut subblock_clients,
                            )
                            .await;

                            let report = proving_msg.fetch_report;
                            info!(
                                "proving-client: save block {} as the current proving block in progress",
                                report.block_number,
                            );
                            // save the proving inputs for potential retry on timeout
                            last_proving_inputs = Some(proving_msg.proving_inputs);
                            proving_block_report = Some(report);
                        } else {
                            info!(
                                "proving-client: save proving request of block {} to the pending queue",
                                proving_msg.fetch_report.block_number,
                            );
                            pending_msgs.push_back(proving_msg);
                        }
                    }
                    Ok(Ok(BlockMsg::Proved(proved_msg))) => {
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
                                proving_msg.proving_inputs.clone(),
                                &mut agg_client,
                                &mut subblock_clients,
                            )
                            .await;

                            let report = proving_msg.fetch_report;
                            info!(
                                "proving-client: save block {} as the current proving block in progress",
                                report.block_number,
                            );
                            // save the proving inputs for potential retry on timeout
                            last_proving_inputs = Some(proving_msg.proving_inputs);
                            proving_block_report = Some(report);
                        }
                    }
                    Err(_) => {
                        if let Some(_report) = &proving_block_report {
                            let block_number = _report.block_number;
                            warn!("proving-client: proving timeout for block {block_number}");
                            warn!(
                                "proving-client: attempting to restart docker containers and retry"
                            );

                            // Step 1: Restart docker containers using the retry script
                            let retry_result = Command::new("./scripts/docker-multi-control.sh")
                                .arg("retry")
                                .status()
                                .await;

                            match retry_result {
                                Ok(status) if status.success() => {
                                    info!(
                                        "proving-client: docker containers restarted successfully"
                                    );
                                }
                                Ok(status) => {
                                    error!(
                                        "proving-client: docker retry script failed with exit code: {:?}",
                                        status.code()
                                    );
                                    panic!(
                                        "proving-client: cannot recover from docker restart failure - manual intervention required"
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "proving-client: failed to execute docker retry script: {}",
                                        e
                                    );
                                    panic!(
                                        "proving-client: cannot recover from docker restart failure - manual intervention required"
                                    );
                                }
                            }

                            // Step 2: Wait for containers to fully initialize
                            info!(
                                "proving-client: waiting {}s for docker containers to initialize",
                                DOCKER_RETRY_WAIT_SECONDS
                            );
                            sleep(Duration::from_secs(DOCKER_RETRY_WAIT_SECONDS)).await;

                            // Step 3: Reinitialize aggregator and subblock clients
                            info!("proving-client: reinitializing aggregator and subblock clients");
                            agg_client = self.init_agg_proving_client(&token).await;
                            subblock_clients = self.init_subblock_proving_clients(&token).await;

                            // Step 4: Resend the last proving inputs to retry the failed block
                            if let Some(ref inputs) = last_proving_inputs {
                                info!(
                                    "proving-client: resending proving inputs for block {}",
                                    block_number
                                );
                                send_proving_inputs(
                                    inputs.clone(),
                                    &mut agg_client,
                                    &mut subblock_clients,
                                )
                                .await;
                                info!(
                                    "proving-client: proving inputs resent, continuing to wait for proof"
                                );
                            } else {
                                error!("proving-client: no proving inputs saved for retry");
                                panic!("proving-client: cannot retry without proving inputs");
                            }
                        }
                    }
                    _ => {
                        error!("proving-client: received an error message {msg:?}");
                        break;
                    }
                }
            }
            info!("proving-client: stopped");
        })
    }

    // initialize a aggregator proving client
    pub async fn init_agg_proving_client(
        &self,
        cancellation_token: &CancellationToken,
    ) -> AggregatorClient<Channel> {
        let max_msg_bytes = self.config.max_msg_bytes;
        let agg_url = self.config.agg_url.clone();

        loop {
            // Check for cancellation first
            if cancellation_token.is_cancelled() {
                info!(
                    "proving-client: cancellation requested, stopping aggregator client initialization"
                );
                panic!("proving-client: cancelled during aggregator client initialization");
            }

            // Try to connect
            match AggregatorClient::connect(agg_url.to_string()).await {
                Ok(client) => {
                    info!("proving-client: successfully connected to aggregator at {agg_url}");
                    return client
                        .max_encoding_message_size(max_msg_bytes)
                        .max_decoding_message_size(max_msg_bytes)
                        .accept_compressed(CompressionEncoding::Zstd)
                        .send_compressed(CompressionEncoding::Zstd);
                }
                Err(e) => {
                    warn!("proving-client: failed to connect to aggregator at {agg_url}: {e}");
                    warn!(
                        "proving-client: retrying in {}s",
                        CLIENT_RETRY_INTERVAL_SECONDS
                    );
                }
            }

            // Wait with cancellation support
            select! {
                _ = cancellation_token.cancelled() => {
                    info!("proving-client: cancellation requested, stopping aggregator client initialization");
                    panic!("proving-client: cancelled during aggregator client initialization");
                }
                _ = sleep(Duration::from_secs(CLIENT_RETRY_INTERVAL_SECONDS)) => {
                    // Continue to next iteration
                }
            }
        }
    }

    // initialize subblock proving clients
    pub async fn init_subblock_proving_clients(
        &self,
        cancellation_token: &CancellationToken,
    ) -> Vec<SubblockClient<Channel>> {
        let max_msg_bytes = self.config.max_msg_bytes;
        let subblock_urls = &self.config.subblock_urls;
        let mut subblock_clients = Vec::with_capacity(subblock_urls.len());
        for url in subblock_urls {
            let client = loop {
                // Check for cancellation first
                if cancellation_token.is_cancelled() {
                    info!(
                        "proving-client: cancellation requested, stopping subblock client initialization"
                    );
                    panic!("proving-client: cancelled during subblock client initialization");
                }

                // Try to connect
                match SubblockClient::connect(url.to_string()).await {
                    Ok(client) => {
                        info!("proving-client: successfully connected to subblock at {url}");
                        break client
                            .max_encoding_message_size(max_msg_bytes)
                            .max_decoding_message_size(max_msg_bytes)
                            .accept_compressed(CompressionEncoding::Zstd)
                            .send_compressed(CompressionEncoding::Zstd);
                    }
                    Err(e) => {
                        warn!("proving-client: failed to connect to subblock at {url}: {e}");
                        warn!(
                            "proving-client: retrying in {}s",
                            CLIENT_RETRY_INTERVAL_SECONDS
                        );
                    }
                }

                // Wait with cancellation support
                select! {
                    _ = cancellation_token.cancelled() => {
                        info!("proving-client: cancellation requested, stopping subblock client initialization");
                        panic!("proving-client: cancelled during subblock client initialization");
                    }
                    _ = sleep(Duration::from_secs(CLIENT_RETRY_INTERVAL_SECONDS)) => {
                        // Continue to next iteration
                    }
                }
            };

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
    let subblock_client_len = subblock_clients.len();
    assert!(
        num_subblocks <= subblock_clients.len(),
        "proving-client: insufficient subblock proving services",
    );
    let num_subblocks = num_subblocks as u32;

    // TODO: check if this could be changed to run futures in parallel
    info!("proving-client: requesting with the aggregator input of block {block_number}");
    let req = ProveAggregationRequest {
        block_number,
        num_subblocks,
        subblock_public_values: proving_inputs.subblock_public_values,
        input: proving_inputs.agg_input,
    };

    // Retry logic for aggregator request
    let mut retry_count = 0;
    loop {
        match agg_client.prove_aggregation(req.clone()).await {
            Ok(_) => {
                if retry_count > 0 {
                    info!(
                        "proving-client: aggregator request succeeded after {retry_count} retries"
                    );
                }
                break;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > MAX_PROVING_REQUEST_RETRIES {
                    error!(
                        "proving-client: failed to request with the aggregator input after {MAX_PROVING_REQUEST_RETRIES} retries: {e}"
                    );
                    panic!("proving-client: failed to request with the aggregator input: {e}");
                }
                warn!(
                    "proving-client: aggregator request failed (attempt {retry_count}/{MAX_PROVING_REQUEST_RETRIES}): {e}"
                );
                warn!(
                    "proving-client: retrying in {}s",
                    PROVING_REQUEST_RETRY_INTERVAL_SECONDS
                );
                sleep(Duration::from_secs(PROVING_REQUEST_RETRY_INTERVAL_SECONDS)).await;
            }
        }
    }

    // TRICKY: aggregator service needs the all subblock services ready, even if the subblock
    // inputs are insufficient
    let mut subblock_inputs = proving_inputs.subblock_inputs;
    if subblock_inputs.len() < subblock_client_len {
        let default_input = subblock_inputs[0].clone();
        subblock_inputs.resize(subblock_client_len, default_input);
    }

    for (i, (client, input)) in subblock_clients
        .iter_mut()
        .zip_eq(subblock_inputs.into_iter())
        .enumerate()
    {
        info!("proving-client: requesting with the {i}-th subblock input of block {block_number}");
        let req = ProveSubblockRequest {
            block_number,
            num_subblocks,
            subblock_index: i as u32,
            input,
        };

        // Retry logic for subblock request
        let mut retry_count = 0;
        loop {
            match client.prove_subblock(req.clone()).await {
                Ok(_) => {
                    if retry_count > 0 {
                        info!(
                            "proving-client: subblock {i} request succeeded after {retry_count} retries"
                        );
                    }
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count > MAX_PROVING_REQUEST_RETRIES {
                        error!(
                            "proving-client: failed to request with the subblock {i} input after {MAX_PROVING_REQUEST_RETRIES} retries: {e}"
                        );
                        panic!("proving-client: failed to request with the subblock input: {e}");
                    }
                    warn!(
                        "proving-client: subblock {i} request failed (attempt {retry_count}/{MAX_PROVING_REQUEST_RETRIES}): {e}"
                    );
                    warn!(
                        "proving-client: retrying in {}s",
                        PROVING_REQUEST_RETRY_INTERVAL_SECONDS
                    );
                    sleep(Duration::from_secs(PROVING_REQUEST_RETRY_INTERVAL_SECONDS)).await;
                }
            }
        }
    }
}
