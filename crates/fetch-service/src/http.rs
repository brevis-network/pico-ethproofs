use crate::service::FetchService;
use anyhow::Result;
use common::fetch::{
    ProveBlockByNumberParams, ProveLatestBlockParams, ReproduceBlockByNumberParams,
};
use std::sync::Arc;

impl FetchService {
    // handle `prove_block_by_number` HTTP Get requests
    pub fn prove_block_by_number(self: Arc<Self>, params: ProveBlockByNumberParams) -> Result<()> {
        self.comm_sender.send(params.into())?;

        Ok(())
    }

    // handle `prove_latest_block` HTTP Get request
    pub fn prove_latest_block(self: Arc<Self>, params: ProveLatestBlockParams) -> Result<()> {
        self.comm_sender.send(params.into())?;

        Ok(())
    }

    // handle `reproduce_block_by_number` HTTP Get requests
    pub fn reproduce_block_by_number(
        self: Arc<Self>,
        params: ReproduceBlockByNumberParams,
    ) -> Result<()> {
        self.comm_sender.send(params.into())?;

        Ok(())
    }
}
