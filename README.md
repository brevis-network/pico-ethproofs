# Pico Real‑Time Proving for ETH Blocks

## Overview

This repository provides an **all‑in‑one** system that fetches Ethereum blocks from an RPC node, executes them inside the Pico, generates ZK proofs that proves the execution, and collects results in reports. It adopts a **multi‑machine, multi‑GPU Aggregator–Subblock** architecture to achieve extreme performance.

### Architecture

- **Fetch Service** (`fetch-service`, HTTP/WS, default `:8080`): Receives client requests (HTTP) and streams progress/results (WebSocket).
- **Proof Service** (`proof-service`, gRPC, default `:50052`): Serves proving RPCs (either to the real distributed proving cluster or a local mock for testing).
- **Fetcher**: Subscribes to Ethereum blocks via RPC (`RPC_HTTP_URL`, `RPC_WS_URL`), prepares inputs, optionally dumps/loads inputs.
- **Proving Client**: Talks to your distributed prover (Aggregator + Subblocks) via gRPC.
- **Reporter**: Aggregates results and writes CSV reports.
- **Scheduler**: Wires the components above and orchestrates the flow.

### ELF artifacts
- `./data/subblock-elf`
- `./data/aggregator-elf`

---

## Getting Started

### 0) Multi‑machine setup
Follow [docs/multi-machine-setup.md](./docs/multi-machine-setup.md) to configure the hardware and software environment (e.g., Rust, NVIDIA drivers, CUDA, etc.) on all machines. This step ensures that Aggregator and Subblock worker machines are correctly provisioned and ready for proving tasks.

### 1) Start `eth-proofs` server

* For **mock proving**, you can directly start the `eth-proofs` server.
* For **real proving**, first follow [docs/reproduce-01-sep-2025.md](./docs/reproduce-01-sep-2025.md) to launch the Aggregator and Subblock worker machines.
  This process will provide you with `PROVING_AGG_URL` and `PROVING_SUBBLOCK_URLS` required in the next step.

Set environment variables (example):

```bash
export RUST_LOG=info
export RPC_HTTP_URL="YOUR_ETH_RPC_HTTP_URL"
export RPC_WS_URL="YOUR_ETH_RPC_WS_URL"

export PROVING_AGG_URL="<AGG_URL from multi-machine setup>"
export PROVING_SUBBLOCK_URLS="<SUBBLOCK_URL1,SUBBLOCK_URL2,... from multi-machine setup>"
```

Run the server (with optional input dump/load for reproduction and a mock mode for testing):

```bash
cargo run -r --bin eth-proofs -- \
  --input-dump-dir block-dump-dir \
  --input-load-dir block-dump-dir \
  --is-mock-proving
```

#### `eth-proofs` service flags
The server wires up **Fetch Service**, **Proof Service**, **Fetcher**, **Proving Client**, **Reporter**, and the **Scheduler**. Key flags/environment variables:

| Flag / Env | Type | Default | Description |
|---|---|---:|---|
| `--is-mock-proving` | bool | `false` | Enable **local mock** proving server (testing). When enabled, `PROVING_*` URLs are auto‑set to the mock. |
| `--is-input-emulated` | bool | `false` | Check the generated inputs by emulation. |
| `--input-dump-dir` | path | – | Base dir to **save** generated inputs. |
| `--input-load-dir` | path | – | Base dir to **load** inputs for **reproduction** (can be same as dump dir). |
| `RPC_HTTP_URL` / `--rpc-http-url` | url | – | Ethereum RPC **HTTP** URL. |
| `RPC_WS_URL` / `--rpc-ws-url` | url | – | Ethereum RPC **WebSocket** URL. |
| `SUBBLOCK_ELF_PATH` / `--subblock-elf-path` | path | `data/subblock-elf` | Subblock ELF path. |
| `AGG_ELF_PATH` / `--agg-elf-path` | path | `data/aggregator-elf` | Aggregator ELF path. |
| `FETCH_SERVICE_ADDR` / `--fetch-service-addr` | addr | `[::]:8080` | Fetch service bind address (HTTP + WS). |
| `PROOF_SERVICE_ADDR` / `--proof-service-addr` | addr | `[::]:50052` | Proof service gRPC bind address. |
| `MAX_GRPC_MSG_BYTES` / `--max-grpc-msg-bytes` | usize | `1073741824` | Max gRPC message size. |
| `PROVING_AGG_URL` / `--proving-agg-url` | url | – | Aggregator proving gRPC URL (required unless `--is-mock-proving`). |
| `PROVING_SUBBLOCK_URLS` / `--proving-subblock-urls` | csv urls | – | Comma‑separated Subblock proving gRPC URLs (required unless `--is-mock-proving`). |

**HTTP/WS endpoints (Fetch Service, default `:8080`)**
- HTTP: `http://127.0.0.1:8080`
- WS:   `ws://127.0.0.1:8080`

### 2) Start a client (three modes)
The server in step 1 accepts these **HTTP** requests, and progress/completion is streamed over **WebSocket**. Three client binaries are provided to wrap these calls and optionally write a CSV report.

#### Mode A — Prove by block number
HTTP:
```
http://127.0.0.1:8080/prove_block_by_number?start_block_num=23264565&count=100
```
CLI:
```bash
RUST_LOG=debug cargo run -r --bin prove-block-by-number -- \
  --start-block-num 23264565 \
  --count 100
```
Client flags:
- `--start-block-num <u64>`: first block to prove
- `--count <u64>=1`: number of blocks
- `--report-path <path>=proving_report.csv`
- `--http-url <url>=http://127.0.0.1:8080`
- `--ws-url <url>=ws://127.0.0.1:8080`

#### Mode B — Prove latest blocks
HTTP:
```
http://127.0.0.1:8080/prove_latest_block?count=100
```
CLI:
```bash
cargo run -r --bin prove-latest-block -- --count 100
```
Client flags:
- `--count <u64>=1`: number of latest blocks
- `--report-path`, `--http-url`, `--ws-url` as above

#### Mode C — Reproduce results for ETH blocks on Sep. 01, 2025
HTTP:
```
http://127.0.0.1:8080/reproduce_block_by_number?start_block_num=23264565&count=7200
```
CLI:
```bash
cargo run -r --bin reproduce-block-by-number -- \
  --start-block-num 23264565 \
  --count 7200
```
Client flags:
- `--start-block-num <u64>`
- `--count <u64>=1`
- `--report-path`, `--http-url`, `--ws-url` as above

> **Reproduction**: With Mode C and [docs/reproduce-01-sep-2025.md](./docs/reproduce-01-sep-2025.md), you can reproduce Pico’s reported Real-Time-Proving results on blocks on **Sep. 01, 2025** from block inputs dumped on your side.


## Security

This repository **has not undergone a security audit**. It is not recommended for use in production environments.


## Acknowledgements

- Thanks to **[ethproofs.org](https://ethproofs.org/)** for providing a platform for Ethereum block proving and inspiring real-time proving efforts.  
- Thanks to **[paradigmxyz/reth](https://github.com/paradigmxyz/reth)** for Rust Ethereum support. The ELF files under `./data` are adapted from Reth and refactored into an Aggregator–Subblock architecture.
- Inspired by **[succinctlabs/rsp-subblock](https://github.com/succinctlabs/rsp-subblock)** for its Subblock design.


## License

This project is licensed under the terms described in [LICENSE.md](LICENSE.md).
