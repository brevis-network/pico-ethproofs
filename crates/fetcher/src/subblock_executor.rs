use crate::config::BlockFetcherConfig;
use alloy_provider::RootProvider;
use anyhow::Result;
use common::inputs::ProvingInputs;
use itertools::Itertools;
use pico_sdk::{HashableKey, client::DefaultProverClient};
use rsp_client_executor::{ChainVariant, io::SubblockHostOutput};
use rsp_host_executor::HostExecutor;
use std::{fs, path::PathBuf, sync::Arc};
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
        // create block dump dir if base dir is specified in configuration
        let block_dump_dir = self
            .config
            .input_dump_dir
            .as_ref()
            .map(|dir| dir.join(block_number.to_string()));
        if let Some(block_dump_dir) = &block_dump_dir {
            fs::create_dir_all(block_dump_dir)?;
        }

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
            &block_dump_dir,
        );

        // generate the aggregation input
        info!("subblock-executor: generating aggregator input for block {block_number}");
        let agg_input = generate_agg_input(
            self.config.is_input_emulated,
            &subblock_output,
            agg_prover_client,
            subblock_vk_hash,
            &block_dump_dir,
        );

        Ok(ProvingInputs::new(block_number, agg_input, subblock_inputs))
    }
}

// generate the subblock inputs
fn generate_subblock_inputs(
    is_input_emulated: bool,
    subblock_output: &SubblockHostOutput,
    subblock_prover_client: DefaultProverClient,
    block_dump_dir: &Option<PathBuf>,
) -> Vec<Vec<u8>> {
    subblock_output
        .subblock_inputs
        .iter()
        .zip_eq(subblock_output.subblock_parent_states.iter())
        .enumerate()
        .map(|(i, (input, parent_state))| {
            // generate subblock stdin builder
            let mut stdin_builder = subblock_prover_client.new_stdin_builder();
            stdin_builder.write(input);
            stdin_builder.write_slice(parent_state);

            // emulate the subblock with generated stdin builder if the flag is specified
            if is_input_emulated {
                subblock_prover_client.emulate(stdin_builder.clone());
            }

            // serialize the stdin builder
            let encoded_input = bincode::serialize(&stdin_builder)
                .expect("subblock-executor: failed to serialize subblock stdin builder");

            // save serialized stdin builder if the dump dir is specified
            if let Some(block_dump_dir) = block_dump_dir {
                let file_path = block_dump_dir.join(format!("subblock_stdin_builder_{i}.bin"));
                fs::write(file_path, &encoded_input)
                    .expect("subblock-executor: failed to dump subblock input");
            }

            encoded_input
        })
        .collect()
}

// generate the aggregation input
fn generate_agg_input(
    is_input_emulated: bool,
    subblock_output: &SubblockHostOutput,
    agg_prover_client: DefaultProverClient,
    subblock_vk_hash: [u32; 8],
    block_dump_dir: &Option<PathBuf>,
) -> Vec<u8> {
    // construct aggregator public values
    let mut public_values = vec![];
    for (input, output) in subblock_output
        .subblock_inputs
        .iter()
        .zip_eq(subblock_output.subblock_outputs.iter())
    {
        let mut subblock_public_values = vec![];
        bincode::serialize_into(&mut subblock_public_values, input)
            .expect("subblock-executor: failed to serialize subblock input into public values");
        bincode::serialize_into(&mut subblock_public_values, output)
            .expect("subblock-executor: failed to serialize subblock output into public values");
        public_values.push(subblock_public_values);
    }

    // generate aggregator stdin builder
    let mut stdin_builder = agg_prover_client.new_stdin_builder();
    stdin_builder.write::<Vec<Vec<u8>>>(&public_values);
    stdin_builder.write::<[u32; 8]>(&subblock_vk_hash);
    stdin_builder.write(&subblock_output.agg_input);
    stdin_builder.write(&subblock_output.agg_input.parent_header().state_root);

    // emulate the aggregator with generated stdin builder if the flag is specified
    if is_input_emulated {
        agg_prover_client.emulate(stdin_builder.clone());
    }

    // serialize the stdin builder
    let encoded_input = bincode::serialize(&stdin_builder)
        .expect("subblock-executor: failed to serialize aggregator stdin builder");

    // save serialized stdin builder if the dump dir is specified
    if let Some(block_dump_dir) = block_dump_dir {
        let file_path = block_dump_dir.join("aggregator_stdin_builder.bin");
        fs::write(file_path, &encoded_input)
            .expect("subblock-executor: failed to dump subblock input");
    }

    encoded_input
}
