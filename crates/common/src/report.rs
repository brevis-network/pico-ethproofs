use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fmt, fs::OpenOptions, io::Write, path::Path, sync::Arc};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BlockProvingReport {
    // identify if proving is success
    pub success: bool,

    // block number
    pub block_number: u64,

    // seconds of block timestamp extracted from the block header
    pub block_timestamp: u64,

    // emulation cycles
    pub cycles: u64,

    // milliseconds of proving time
    pub proving_milliseconds: u64,

    // milliseconds of fetching and preparing block input data
    pub fetch_milliseconds: u64,

    // milliseconds of fetching start timestamp
    pub fetch_start_timestamp: u64,

    // milliseconds of fetching end timestamp
    pub fetch_end_timestamp: u64,

    // milliseconds of proving start timestamp
    pub proving_start_timestamp: u64,

    // milliseconds of proving end timestamp
    pub proving_end_timestamp: u64,

    // bincode serialized proof bytes
    pub proof: Option<Arc<Vec<u8>>>,
}

impl fmt::Display for BlockProvingReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Block #{} | success: {} | cycles: {} | proving: {} ms | fetch: {} ms",
            self.block_number,
            self.success,
            self.cycles,
            self.proving_milliseconds,
            self.fetch_milliseconds,
        )
    }
}

impl BlockProvingReport {
    // initialize a report after fetching block data
    pub fn new(
        block_number: u64,
        block_timestamp: u64,
        fetch_milliseconds: u64,
        fetch_start_timestamp: u64,
        fetch_end_timestamp: u64,
    ) -> Self {
        Self {
            block_number,
            block_timestamp,
            fetch_milliseconds,
            fetch_start_timestamp,
            fetch_end_timestamp,
            ..Default::default()
        }
    }

    // set proving success
    pub fn on_proving_success(
        &mut self,
        cycles: u64,
        proving_milliseconds: u64,
        proving_start_timestamp: u64,
        proving_end_timestamp: u64,
        proof: Arc<Vec<u8>>,
    ) {
        self.success = true;
        self.cycles = cycles;
        self.proving_milliseconds = proving_milliseconds;
        self.proving_start_timestamp = proving_start_timestamp;
        self.proving_end_timestamp = proving_end_timestamp;
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
                concat!(
                    "block_number,success,cycles,",
                    "total_secs (proving_end - block_timestamp),",
                    "return_proving_secs,whole_proving_secs,",
                    "fetch_secs,whole_fetch_secs,",
                    "running_secs (proving_end - fetch_start),",
                    "fetch_interval (fetch_start - block_timestamp),",
                    "proving_interval (proving_start - fetch_end)",
                ),
            )?;
        }

        let total_secs = self.proving_end_timestamp as f64 / 1000.0 - self.block_timestamp as f64;
        let return_proving_secs = self.proving_milliseconds as f64 / 1000.0;
        let whole_proving_secs =
            (self.proving_end_timestamp - self.proving_start_timestamp) as f64 / 1000.0;
        let fetch_secs = self.fetch_milliseconds as f64 / 1000.0;
        let whole_fetch_secs =
            (self.fetch_end_timestamp - self.fetch_start_timestamp) as f64 / 1000.0;
        let running_secs =
            (self.proving_end_timestamp - self.fetch_start_timestamp) as f64 / 1000.0;
        let fetch_interval =
            self.fetch_start_timestamp as f64 / 1000.0 - self.block_timestamp as f64;
        let proving_interval =
            (self.proving_start_timestamp - self.fetch_end_timestamp) as f64 / 1000.0;

        writeln!(
            file,
            "{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            self.block_number,
            self.success,
            self.cycles,
            total_secs,
            return_proving_secs,
            whole_proving_secs,
            fetch_secs,
            whole_fetch_secs,
            running_secs,
            fetch_interval,
            proving_interval,
        )?;

        Ok(())
    }
}
