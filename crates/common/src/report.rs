use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs::OpenOptions, io::Write, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BlockProvingReport {
    // identify if proving is success
    pub success: bool,

    // emulation cycles
    pub cycles: u64,

    // seconds of proving time
    pub proving_seconds: u64,

    // seconds of fetching and preparing block input data
    pub data_fetch_seconds: u64,

    // seconds of total time
    pub total_seconds: u64,

    // bincode serialized proof bytes
    pub proofs: Option<Vec<u8>>,
}

impl BlockProvingReport {
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
                "success,cycles,proving_seconds,data_fetch_seconds,total_seconds",
            )?;
        }

        writeln!(
            file,
            "{},{},{},{},{}",
            self.success,
            self.cycles,
            self.proving_seconds,
            self.data_fetch_seconds,
            self.total_seconds
        )?;

        Ok(())
    }
}
