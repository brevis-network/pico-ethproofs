use common::{
    channel::{DuplexUnboundedEndpoint, Receiver, Sender},
    fetch::{ProveBlockByNumberParams, ProveLatestBlockParams},
    report::BlockProvingReport,
};
use derive_more::Constructor;
use proof_proto::CompleteProvingRequest;
use std::sync::Arc;

// default value of `count` parameter
const DEFAULT_PARAM_COUNT: u64 = 1;

// block message transmitted between multiple threads
#[derive(Clone, Debug)]
pub enum BlockMsg {
    // monitor block proving message
    Watch(WatchMsg),

    // fetch request message
    Fetch(FetchMsg),

    // proving request message
    Proving(ProvingMsg),

    // proving result message
    Proved(ProvedMsg),

    // block report message
    Report(ReportMsg),
}

impl From<ProveBlockByNumberParams> for BlockMsg {
    fn from(params: ProveBlockByNumberParams) -> Self {
        let fetch_msg = FetchMsg::FromStart {
            start_block_number: params.start_block_num,
            count: params.count.unwrap_or(DEFAULT_PARAM_COUNT),
        };

        Self::Fetch(fetch_msg)
    }
}

impl From<ProveLatestBlockParams> for BlockMsg {
    fn from(params: ProveLatestBlockParams) -> Self {
        let fetch_msg = FetchMsg::Latest {
            count: params.count.unwrap_or(DEFAULT_PARAM_COUNT),
        };

        Self::Fetch(fetch_msg)
    }
}

// monitor block proving message
#[derive(Clone, Constructor, Debug)]
pub struct WatchMsg {
    // notifier for sending the block proving report
    pub sender: Arc<BlockMsgSender>,
}

// fetch request message
#[derive(Clone, Debug)]
pub enum FetchMsg {
    // fetch number of blocks starting from a specified block number
    FromStart { start_block_number: u64, count: u64 },

    // fetch number of latest blocks
    Latest { count: u64 },
}

// proving request message
#[derive(Clone, Constructor, Debug)]
pub struct ProvingMsg {
    // block number to prove
    pub block_number: u64,

    // bincode serialized aggregation stdin builder
    pub agg_input: Vec<u8>,

    // bincode serialized multiple subblock stdin builders
    pub subblock_inputs: Vec<Vec<u8>>,
}

pub type ProvedMsg = CompleteProvingRequest;
pub type ReportMsg = BlockProvingReport;

pub type BlockMsgSender = Sender<BlockMsg>;
pub type BlockMsgReceiver = Receiver<BlockMsg>;
pub type BlockMsgEndpoint = DuplexUnboundedEndpoint<BlockMsg, BlockMsg>;
