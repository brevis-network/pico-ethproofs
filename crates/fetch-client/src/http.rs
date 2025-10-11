use anyhow::Result;
use common::fetch::{
    HTTP_PROVE_BLOCK_BY_NUMBER_PATH, HTTP_PROVE_LATEST_BLOCK_PATH,
    HTTP_REPRODUCE_BLOCK_BY_NUMBER_PATH, ProveBlockByNumberParams, ProveLatestBlockParams,
    ReproduceBlockByNumberParams,
};
use reqwest::{Client, Url};
use tracing::info;

// send a http request:
// `http://HTTP_URL/prove_block_by_number?start_block_num=START_BLOCK_NUM&count=COUNT`
pub async fn prove_block_by_number(
    http_url: &Url,
    params: &ProveBlockByNumberParams,
) -> Result<()> {
    let url = http_url.join(HTTP_PROVE_BLOCK_BY_NUMBER_PATH)?;
    let params = params.to_hash_map();

    info!("sending HTTP request: url = {url}, params = {params:?}");
    Client::new().get(url).query(&params).send().await?;

    Ok(())
}

// send a http request:
// `http://HTTP_URL/prove_latest_block?count=COUNT`
pub async fn prove_latest_block(http_url: &Url, params: &ProveLatestBlockParams) -> Result<()> {
    let url = http_url.join(HTTP_PROVE_LATEST_BLOCK_PATH)?;
    let params = params.to_hash_map();

    info!("sending HTTP request: url = {url}, params = {params:?}");
    Client::new().get(url).query(&params).send().await?;

    Ok(())
}

// send a http request:
// `http://HTTP_URL/reproduce_block_by_number?start_block_num=START_BLOCK_NUM&count=COUNT`
pub async fn reproduce_block_by_number(
    http_url: &Url,
    params: &ReproduceBlockByNumberParams,
) -> Result<()> {
    let url = http_url.join(HTTP_REPRODUCE_BLOCK_BY_NUMBER_PATH)?;
    let params = params.to_hash_map();

    info!("sending HTTP request: url = {url}, params = {params:?}");
    Client::new().get(url).query(&params).send().await?;

    Ok(())
}
