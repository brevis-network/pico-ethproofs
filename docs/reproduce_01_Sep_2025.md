# Reproducing Proving Blocks of Sep 01 2025

This document describes how to reproduce proving the blocks of Sep 01 2025 by using 1 CPU machine and 8 GPU machines.

---

## 1. System Setup

### Machines
- **1 CPU machine:** run a pico-ethproofs server and a client for fetching the blocks and triggering the whole proving process, setup a normal [Rust](https://rust-lang.org/) development machine.
- **8 GPU machines:** run the Aggregator and Subblock docker containers as a proving cluster, reference [multi-machine-setup.md](./multi-machine-setup.md) for these GPU machine setup.

### Install Docker

- TODO

---

## 2. Launch Docker Containers

### Aggregator Machine

- TODO

### Subblock Machines

- TODO

---

## 3. Download Subblock Data

On the CPU instance:

1. Create a target folder:
```bash
mkdir -p /home/ubuntu/subblocks-20250901
cd /home/ubuntu/subblocks-20250901
```

2. Download the all block inputs:
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23264565%2B235.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23264800%2B200.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23265000%2B200.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23265200%2B200.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23265400%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23265700%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23265800%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23266100%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23266200%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23266600%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23267000%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23267400%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23267800%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23268200%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23268600%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23269000%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23269400%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23269700%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23269800%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23270100%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23270200%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23270500%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23270600%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23270900%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23271000%2B300.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23271300%2B100.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23271400%2B330.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/subblocks-20250901/23271730%2B35.tar.gz
```

---

## 4. Clone Repository

```bash
git clone https://github.com/brevis-network/pico-ethproofs.git
cd pico-ethproofs
```

---

## 5. Start `pico-ethproofs` Server

### Set Environment Variables
```bash
export RUST_LOG=info
export RPC_HTTP_URL="YOUR_RPC_HTTP_URL"
export RPC_WS_URL="YOUR_RPC_WS_URL"

export PROVING_AGG_URL="<AGG_URL from multi-machine setup>"
export PROVING_SUBBLOCK_URLS="<SUBBLOCK_URL1,SUBBLOCK_URL2,... from multi-machine setup>"
```

### Run Server
```bash
cargo run -r --bin eth-proofs -- \
  --input-load-dir /home/ubuntu/subblocks-20250901
```

---

## 6. Start `ethproofs-client`

Open another terminal:
```bash
cargo run -r --bin reproduce-block-by-number -- \
  --start-block-num 23264565 \
  --count 7200
```

---

## 7. Verify Logs and Output

- Logs will show progress for each proving task  
- Wait until **all proving results** are saved into `proving_report.csv` in the working directory  

---

## Notes / Tips

- Ensure **network connectivity** between aggregator and subblock machines  
- Docker containers must have **GPU access** if proving relies on GPU  
- Adjust environment variables according to your setup
