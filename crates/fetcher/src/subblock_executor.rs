use crate::config::BlockFetcherConfig;
use alloy_provider::RootProvider;
use anyhow::Result;
use common::inputs::ProvingInputs;
use itertools::Itertools;
use pico_sdk::{
    DIGEST_SIZE, EmulatorStdinBuilder, KoalaBearPoseidon2, client::DefaultProverClient,
};
use rsp_client_executor::{ChainVariant, ClientExecutor, EthereumVariant, io::SubblockHostOutput};
use rsp_host_executor::HostExecutor;
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
use tracing::info;

// subblock executor for generating subblock and aggregation inputs
pub struct SubblockExecutor {
    // fetcher configuration
    config: Arc<BlockFetcherConfig>,

    // rsp-subblock executor
    executor: HostExecutor<RootProvider>,

    // hash of subblock vk
    subblock_vk_digest: [u32; DIGEST_SIZE],
}

impl SubblockExecutor {
    pub fn new(config: Arc<BlockFetcherConfig>) -> Self {
        // create rsp-subblock executor
        let basic_provider = RootProvider::new_http(config.basic_rpc_http_url.clone());
        let debug_provider = RootProvider::new_http(config.debug_rpc_http_url.clone());
        let executor = HostExecutor::new(basic_provider, debug_provider);

        // read and deserialize the subblock verification key digest
        let subblock_vk_digest = {
            let data = fs::read(&config.subblock_vk_digest_path).expect(
                "subblock-executor: failed to read subblock verification key digest from the file",
            );
            bincode::deserialize(&data)
                .expect("subblock-executor: failed to deserialize subblock verification key digest")
        };

        Self {
            config,
            executor,
            subblock_vk_digest,
        }
    }

    // generate subblock and aggregation inputs
    pub async fn generate_inputs(
        &self,
        is_latest_block: bool,
        block_number: u64,
    ) -> Result<ProvingInputs> {
        // benchmark the whole running time
        let total_start = Instant::now();

        let start = Instant::now();
        let use_execution_witness = cfg!(feature = "latest-execution-witness") && is_latest_block;
        info!(
            "subblock-executor: fetching block {block_number} with use_execution_witness={use_execution_witness}",
        );
        let subblock_output = self
            .executor
            .execute_subblock(
                use_execution_witness,
                block_number,
                ChainVariant::Ethereum,
                self.config.input_dump_dir.clone(),
            )
            .await?;
        info!(
            "[bench] subblock-executor: execute subblock: {:.3?}",
            start.elapsed(),
        );

        // generate the subblock inputs
        let start = Instant::now();
        info!("subblock-executor: generating subblock inputs for block {block_number}");
        let subblock_inputs = generate_subblock_inputs(
            self.config.is_input_emulated,
            &self.config.subblock_elf_path,
            &subblock_output,
        );
        info!(
            "[bench] subblock-executor: generate subblock inputs: {:.3?}",
            start.elapsed(),
        );

        // generate the subblock public values
        let start = Instant::now();
        let subblock_public_values = generate_subblock_public_values(&subblock_output);
        info!(
            "[bench] subblock-executor: generate subblock public values: {:.3?}",
            start.elapsed(),
        );

        // generate the aggregation input
        let start = Instant::now();
        info!("subblock-executor: generating aggregator input for block {block_number}");
        let agg_input = generate_agg_input(
            self.config.is_input_emulated,
            &self.config.agg_elf_path,
            &subblock_output,
            &self.subblock_vk_digest,
            &subblock_public_values,
        );
        info!(
            "[bench] subblock-executor: generate aggregator input: {:.3?}",
            start.elapsed(),
        );

        let start = Instant::now();
        let subblock_public_values = bincode::serialize(&subblock_public_values)
            .expect("subblock-executor: failed to serialize subblock public values");

        let proving_inputs = ProvingInputs::new(
            block_number,
            subblock_public_values,
            agg_input,
            subblock_inputs,
        );
        info!(
            "[bench] subblock-executor: construct proving inputs: {:.3?}",
            start.elapsed(),
        );

        if let Some(dir) = &self.config.input_dump_dir {
            // save proving inputs to the directory
            let start = Instant::now();
            proving_inputs
                .dump_to_dir(dir)
                .expect("subblock-executor: failed to dump the block proving inputs");
            info!(
                "[bench] subblock-executor: dump proving inputs: {:.3?}",
                start.elapsed(),
            );
        }

        info!(
            "[bench] subblock-executor: generate_inputs total time: {:.3?}",
            total_start.elapsed(),
        );

        Ok(proving_inputs)
    }
}

// generate the subblock inputs
fn generate_subblock_inputs(
    is_input_emulated: bool,
    subblock_elf_path: &PathBuf,
    subblock_output: &SubblockHostOutput,
) -> Vec<Vec<u8>> {
    subblock_output
        .subblock_inputs
        .iter()
        .zip_eq(subblock_output.subblock_parent_states.iter())
        .map(|(input, parent_state)| {
            // generate subblock stdin builder
            let mut stdin_builder = EmulatorStdinBuilder::<Vec<u8>, KoalaBearPoseidon2>::default();
            stdin_builder.write(input);
            stdin_builder.write_slice(parent_state);

            // emulate the subblock with generated stdin builder if the flag is specified
            if is_input_emulated {
                let subblock_elf = fs::read(subblock_elf_path)
                    .expect("subblock-executor: failed to read file of subblock ELF");
                let subblock_prover_client = DefaultProverClient::new(&subblock_elf);
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
    agg_elf_path: &PathBuf,
    subblock_output: &SubblockHostOutput,
    subblock_vk_digest: &[u32; 8],
    subblock_public_values: &Vec<Vec<u8>>,
) -> Vec<u8> {
    // generate aggregator stdin builder
    let mut stdin_builder = EmulatorStdinBuilder::<Vec<u8>, KoalaBearPoseidon2>::default();
    stdin_builder.write::<Vec<Vec<u8>>>(subblock_public_values);
    stdin_builder.write::<[u32; 8]>(subblock_vk_digest);
    stdin_builder.write(&subblock_output.agg_input);
    stdin_builder.write(&subblock_output.agg_input.parent_header().state_root);

    // emulate the aggregator with generated stdin builder if the flag is specified
    if is_input_emulated {
        // execute aggregation for validation
        ClientExecutor
            .execute_aggregation::<EthereumVariant>(
                subblock_public_values.clone(),
                *subblock_vk_digest,
                subblock_output.agg_input.clone(),
                subblock_output.agg_input.parent_header().state_root,
            )
            .expect("subblock-executor: failed to execute aggregation for validation");

        // emulate for the aggregator input
        let agg_elf = fs::read(agg_elf_path)
            .expect("subblock-executor: failed to read file of aggregator ELF");
        let agg_prover_client = DefaultProverClient::new(&agg_elf);
        agg_prover_client.emulate(stdin_builder.clone());
    }

    // serialize the stdin builder
    bincode::serialize(&stdin_builder)
        .expect("subblock-executor: failed to serialize aggregator stdin builder")
}
