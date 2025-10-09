use derive_more::Constructor;

#[derive(Clone, Constructor, Debug)]
pub struct ProvingInputs {
    // block number to prove
    pub block_number: u64,

    // subblock public values
    pub subblock_public_values: Vec<Vec<u8>>,

    // bincode serialized aggregation stdin builder
    pub agg_input: Vec<u8>,

    // bincode serialized multiple subblock stdin builders
    pub subblock_inputs: Vec<Vec<u8>>,
}
