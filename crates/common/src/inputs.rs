use crate::utils::MAX_NUM_SUBBLOCKS;
use anyhow::{Result, bail};
use derive_more::Constructor;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Constructor, Debug)]
pub struct ProvingInputs {
    // block number to prove
    pub block_number: u64,

    // bincode serialized aggregation stdin builder
    pub agg_input: Vec<u8>,

    // bincode serialized multiple subblock stdin builders
    pub subblock_inputs: Vec<Vec<u8>>,

    // subblock public values
    // it must have the same size as the subblock inputs
    pub subblock_public_values: Vec<Vec<u8>>,
}

impl ProvingInputs {
    // save the proving inputs to a directory
    pub fn dump_to_dir(&self, dir: &Path) -> Result<()> {
        let dir = block_dir(self.block_number, dir);
        fs::create_dir_all(&dir)?;

        // save the subblock public values
        let file_path = dir.join("public_values.bin");
        let subblock_public_values = bincode::serialize(&self.subblock_public_values)?;
        fs::write(file_path, subblock_public_values)?;

        // save the aggregator input
        let file_path = dir.join("final_aggregator_stdin_builder.bin");
        fs::write(file_path, &self.agg_input)?;

        // save the subblock inputs
        for (i, input) in self.subblock_inputs.iter().enumerate() {
            let file_path = dir.join(format!("subblock_stdin_builder_{i}.bin"));
            fs::write(file_path, input)?;
        }

        Ok(())
    }

    // load the proving inputs from a directory
    pub fn load_from_dir(block_number: u64, dir: &Path) -> Result<Self> {
        let dir = block_dir(block_number, dir);
        if !dir.exists() {
            bail!("cannot read proving inputs from {dir:?} since it doesn't exist");
        }

        // save the subblock public values
        let file_path = dir.join("public_values.bin");
        let subblock_public_values = fs::read(file_path)?;
        let subblock_public_values = bincode::deserialize(&subblock_public_values)?;

        // save the aggregator input
        let file_path = dir.join("final_aggregator_stdin_builder.bin");
        let agg_input = fs::read(file_path)?;

        // save the subblock inputs
        let mut subblock_inputs = Vec::with_capacity(MAX_NUM_SUBBLOCKS);
        for i in 0..MAX_NUM_SUBBLOCKS {
            let file_path = dir.join(format!("subblock_stdin_builder_{i}.bin"));
            match fs::read(file_path) {
                Ok(input) => subblock_inputs.push(input),
                Err(_) => break,
            }
        }
        assert!(
            !subblock_inputs.is_empty(),
            "must have one subblock at least",
        );

        Ok(ProvingInputs {
            block_number,
            subblock_public_values,
            agg_input,
            subblock_inputs,
        })
    }
}

// construct the block base directory
fn block_dir(block_number: u64, dir: &Path) -> PathBuf {
    dir.join(format!("block{}", block_number))
        .join("gas10000000")
}
