#![allow(clippy::too_many_arguments)]

use derive_more::Constructor;
use reqwest::Url;
use std::path::PathBuf;

// block fetcher configuration
#[derive(Constructor, Debug)]
pub struct BlockFetcherConfig {
    // identify if should check the generated inputs by emulation
    pub is_input_emulated: bool,

    // base directory for saving input files; nothing will be saved if not specified
    pub input_dump_dir: Option<PathBuf>,

    // base directory for reproducing blocks by loading input files; it could be the same directory
    // as `input_dump_dir`
    pub input_load_dir: Option<PathBuf>,

    // basic http url for common rpc requests
    pub basic_rpc_http_url: Url,

    // debug http url for debug rpc requests; specially for debug_executionWitness
    pub debug_rpc_http_url: Url,

    // websocket url of rpc node
    pub rpc_ws_url: Url,

    // subblock verification key digest file path
    pub subblock_vk_digest_path: PathBuf,

    // subblock elf file path
    pub subblock_elf_path: PathBuf,

    // aggregator elf file path
    pub agg_elf_path: PathBuf,
}
