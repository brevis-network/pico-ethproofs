use crate::config::BlockFetcherConfig;
use alloy_provider::RootProvider;
use anyhow::Result;
use common::inputs::ProvingInputs;
use itertools::Itertools;
use pico_sdk::{HashableKey, client::DefaultProverClient};
use rsp_client_executor::{ChainVariant, io::SubblockHostOutput};
use rsp_host_executor::HostExecutor;
use std::{fs, sync::Arc};
use tracing::info;

// subblock executor for generating subblock and aggregation inputs
pub struct SubblockExecutor {
    // fetcher configuration
    config: Arc<BlockFetcherConfig>,

    // rsp-subblock executor
    executor: HostExecutor<RootProvider>,
}

impl SubblockExecutor {
    pub fn new(config: Arc<BlockFetcherConfig>) -> Self {
        // create rsp-subblock executor
        let provider = RootProvider::new_http(config.rpc_http_url.clone());
        let executor = HostExecutor::new(provider);

        Self { config, executor }
    }

    // generate subblock and aggregation inputs
    pub async fn generate_inputs(&self, block_number: u64) -> Result<ProvingInputs> {
        // fetch eth block data and generate the subblock output
        info!(
            "subblock-executor: fetching and generating subblock output for block {block_number}",
        );
        let subblock_output = self
            .executor
            .execute_subblock(block_number, ChainVariant::Ethereum, None)
            .await?;

        // create subblock and aggregation prover clients
        let subblock_elf = fs::read(&self.config.subblock_elf_path)?;
        let agg_elf = fs::read(&self.config.agg_elf_path)?;
        let subblock_prover_client = DefaultProverClient::new(&subblock_elf);
        let agg_prover_client = DefaultProverClient::new(&agg_elf);
        let subblock_vk_hash = subblock_prover_client.riscv_vk().hash_u32();

        // generate the subblock inputs
        info!("subblock-executor: generating subblock inputs for block {block_number}");
        let subblock_inputs = generate_subblock_inputs(
            self.config.is_input_emulated,
            &subblock_output,
            subblock_prover_client,
        );

        // generate the subblock public values
        let subblock_public_values = generate_subblock_public_values(&subblock_output);

        // generate the aggregation input
        info!("subblock-executor: generating aggregator input for block {block_number}");
        let agg_input = generate_agg_input(
            self.config.is_input_emulated,
            &subblock_output,
            agg_prover_client,
            subblock_vk_hash,
            &subblock_public_values,
        );

        let proving_inputs = ProvingInputs::new(
            block_number,
            agg_input,
            subblock_inputs,
            subblock_public_values,
        );

        if let Some(dir) = &self.config.input_dump_dir {
            // save proving inputs to the directory
            proving_inputs
                .dump_to_dir(dir)
                .expect("subblock-executor: failed to dump the block proving inputs");
        }

        Ok(proving_inputs)
    }
}

// generate the subblock inputs
fn generate_subblock_inputs(
    is_input_emulated: bool,
    subblock_output: &SubblockHostOutput,
    subblock_prover_client: DefaultProverClient,
) -> Vec<Vec<u8>> {
    subblock_output
        .subblock_inputs
        .iter()
        .zip_eq(subblock_output.subblock_parent_states.iter())
        .map(|(input, parent_state)| {
            // generate subblock stdin builder
            let mut stdin_builder = subblock_prover_client.new_stdin_builder();
            stdin_builder.write(input);
            stdin_builder.write_slice(parent_state);

            // emulate the subblock with generated stdin builder if the flag is specified
            if is_input_emulated {
                subblock_prover_client.emulate(stdin_builder.clone());
            }

            // serialize the stdin builder
            bincode::serialize(&stdin_builder)
                .expect("subblock-executor: failed to serialize subblock stdin builder")
        })
        .collect()
}

// generate the subblock public values
fn generate_subblock_public_values(subblock_output: &SubblockHostOutput) -> Vec<Vec<u8>> {
    // construct the public values
    let mut public_values = vec![];
    for (input, output) in subblock_output
        .subblock_inputs
        .iter()
        .zip_eq(subblock_output.subblock_outputs.iter())
    {
        let mut pv = vec![];
        bincode::serialize_into(&mut pv, input)
            .expect("subblock-executor: failed to serialize subblock input into public values");
        bincode::serialize_into(&mut pv, output)
            .expect("subblock-executor: failed to serialize subblock output into public values");
        public_values.push(pv);
    }

    public_values
}

// generate the aggregation input
fn generate_agg_input(
    is_input_emulated: bool,
    subblock_output: &SubblockHostOutput,
    agg_prover_client: DefaultProverClient,
    subblock_vk_hash: [u32; 8],
    subblock_public_values: &Vec<Vec<u8>>,
) -> Vec<u8> {
    // generate aggregator stdin builder
    let mut stdin_builder = agg_prover_client.new_stdin_builder();
    stdin_builder.write::<Vec<Vec<u8>>>(subblock_public_values);
    stdin_builder.write::<[u32; 8]>(&subblock_vk_hash);
    stdin_builder.write(&subblock_output.agg_input);
    stdin_builder.write(&subblock_output.agg_input.parent_header().state_root);

    // emulate the aggregator with generated stdin builder if the flag is specified
    if is_input_emulated {
        agg_prover_client.emulate(stdin_builder.clone());
    }

    // serialize the stdin builder
    bincode::serialize(&stdin_builder)
        .expect("subblock-executor: failed to serialize aggregator stdin builder")
}
