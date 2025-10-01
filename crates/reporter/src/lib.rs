use common::report::BlockProvingReport;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct BlockReporter {
    pub block_reports: HashMap<u64, BlockProvingReport>,
}
