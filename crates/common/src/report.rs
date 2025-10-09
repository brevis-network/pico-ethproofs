use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fmt, fs::OpenOptions, io::Write, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BlockProvingReport {
    // identify if proving is success
    pub success: bool,

    // block number
    pub block_number: u64,

    // emulation cycles
    pub cycles: u64,

    // milliseconds of proving time
    pub proving_milliseconds: u64,

    // milliseconds of fetching and preparing block input data
    pub data_fetch_milliseconds: u64,

    // bincode serialized proof bytes
    pub proof: Option<Vec<u8>>,
}

impl fmt::Display for BlockProvingReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Block #{} | success: {} | cycles: {} | proving: {} ms | data_fetch: {} ms",
            self.block_number,
            self.success,
            self.cycles,
            self.proving_milliseconds,
            self.data_fetch_milliseconds,
        )
    }
}

impl BlockProvingReport {
    // initialize a report after fetching block data
    pub fn new(block_number: u64, data_fetch_milliseconds: u64) -> Self {
        Self {
            block_number,
            data_fetch_milliseconds,
            ..Default::default()
        }
    }

    // set proving success
    pub fn on_proving_success(&mut self, cycles: u64, proving_milliseconds: u64, proof: Vec<u8>) {
        self.success = true;
        self.cycles = cycles;
        self.proving_milliseconds = proving_milliseconds;
        self.proof = Some(proof);
    }

    // set proving failure
    pub fn on_proving_failure(&mut self) {
        self.success = false;
    }

    pub fn append_to_csv<P: AsRef<Path>>(&self, csv_file_path: P) -> Result<()> {
        let file_path = csv_file_path.as_ref();
        let file_exists = file_path.exists();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        if !file_exists {
            writeln!(
                file,
                "block_number,success,cycles,proving_milliseconds,data_fetch_milliseconds",
            )?;
        }

        writeln!(
            file,
            "{},{},{},{},{}",
            self.block_number,
            self.success,
            self.cycles,
            self.proving_milliseconds,
            self.data_fetch_milliseconds,
        )?;

        Ok(())
    }
}
