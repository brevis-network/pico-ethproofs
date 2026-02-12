# Reproducing Results for Proving Blocks on Sep. 01, 2025 (Two-Machine Setup)

This document describes how to reproduce the results for proving of blocks on September 1 2025, using a **two-machine** distributed proving setup.

**Terminology Note:** The following terminology is used throughout this document:
- **Primary machine**: Runs the `primary-node` binary. Acts as both a proving worker and the global scheduler, which maintains the global proof tree and dispatches tasks to both nodes.
- **Secondary machine**: Runs the `secondary-node` binary. Acts as a proving worker that connects to the primary machine to pull and execute proving tasks.

---

## 1. Computing Infrastructure

Two Compute Nodes, each with:
- **CPU:** AMD EPYC 9575F Processor (120 vCPUs per NUMA node, 2 NUMA nodes in total)
- **GPU:** 8 × NVIDIA GeForce RTX 5090
- **RAM:** 462 GB DDR5 Memory

**Networking:** 100Gbps Interconnect between the two nodes.

### NUMA Pinning

For best performance, Docker containers are pinned to a **single** NUMA node. By default, the Makefile binds to **NUMA node 1** (`CPUSET_CPUS=120-239`). Mismatched CPU/memory NUMA placement causes cross-node memory access with roughly 2× latency penalty, which can degrade end-to-end proving time by ~13%.

If your server has a different NUMA topology, determine the correct CPU range on **both** machines:

```bash
numactl --hardware
```

Look at the `node 1 cpus:` line in the output — this is the range to pass as `CPUSET_CPUS`. For example, if the output shows:

```
node 1 cpus: 64 65 66 ... 127
```

Then use `CPUSET_CPUS=64-127` in the `make` commands below.

### Prerequisites

Refer to [multi-machine-setup.md](../docs/multi-machine-setup.md) to configure the hardware and software environment (e.g., NVIDIA drivers, CUDA, Docker with GPU support) on **both** machines.

---

## 2. Download Docker Image

On **both** machines, download and load the Docker image:

```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/primary-secondary/pico-two-machine.tar.gz
gunzip -d pico-two-machine.tar.gz
sudo docker load -i pico-two-machine.tar
```

> [!NOTE]
> This is a single unified Docker image containing both the primary and secondary node binaries. The container automatically starts the correct binary based on the `MACHINE_ROLE` environment variable set in the `.env` file.

---

## 3. Download Program Cache

On **both** machines:

1. Download the program cache:
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/primary-secondary/program_cache.bin.tar.gz
```

2. Extract the archive:
```bash
tar -xzf program_cache.bin.tar.gz
```

---

## 4. Download and Decompress Block Inputs (Stdins)

On **both** machines:

1. Create a target folder:
```bash
mkdir -p /home/ubuntu/reth-pico-20250901
cd /home/ubuntu/reth-pico-20250901
```

**Note:** We recommend at least 300G of disk space available.

2. Download all block inputs:
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23264565%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23264965%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23265365%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23265765%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23266165%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23266565%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23266965%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23267365%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23267765%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23268165%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23268565%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23268965%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23269365%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23269765%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23270165%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23270565%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23270965%2B400.tar.gz
wget https://pico-proofs.s3.us-west-2.amazonaws.com/rsp-20250901/23271365%2B400.tar.gz
```

3. Decompress all downloaded files:
```bash
ls *.tar.gz | xargs -P"$(nproc)" -I{} bash -c 'echo "Extracting {}" && tar xzf "{}"'
```

This provides 7,200 blocks of input data (18 archives × 400 blocks each), covering blocks **23264565** through **23271764**.

---

## 5. Configure Environment Files

All environment variables are pre-configured in the `.env` templates and Docker image defaults. The only required user edit is replacing `<PRIMARY_IP>` in the secondary's `.env` file.

### On the primary machine

```bash
cp .env.primary.template .env.primary
```

No edits needed — defaults are correct.

### On the secondary machine

```bash
cp .env.secondary.template .env.secondary
```

Edit `.env.secondary` and replace `<PRIMARY_IP>` with the primary machine's actual IP address in these three lines:

```
PEER_MACHINE_ADDR=http://<PRIMARY_IP>:50052
ORCHESTRATOR_ADDR=http://<PRIMARY_IP>:50053
GLOBAL_SCHEDULER_ADDR=http://<PRIMARY_IP>:50054
```

---

## 6. Run Docker Containers

> [!IMPORTANT]
> Start the **primary node first**. The secondary node connects to gRPC services running on the primary machine.

### Start Primary Node

On the **primary machine**:

```bash
make up-primary \
  DOCKER="sudo docker" \
  CPUSET_CPUS=120-239 \
  BLOCK_INPUT_DIR=/home/ubuntu/reth-pico-20250901 \
  PROGRAM_CACHE_FILE=/home/ubuntu/program_cache.bin
```

### Start Secondary Node

On the **secondary machine**:

```bash
make up-secondary \
  DOCKER="sudo docker" \
  CPUSET_CPUS=120-239 \
  BLOCK_INPUT_DIR=/home/ubuntu/reth-pico-20250901 \
  PROGRAM_CACHE_FILE=/home/ubuntu/program_cache.bin
```

---

## 7. Verify Logs and Collect Results

Monitor the proving process:

```bash
# Follow primary container logs
make logs-primary DOCKER="sudo docker"

# Follow secondary container logs
make logs-secondary DOCKER="sudo docker"

# Check container status
make status DOCKER="sudo docker"
```

- The primary node logs will show progress for each block being proved.
- The secondary node logs will show tasks being pulled from the global scheduler and executed.
- Wait until all 7,200 blocks are processed.

### Collect Results

Once proving is complete, extract the primary container's logs and analyze them to produce a CSV report:

```bash
# 1. Save primary container logs to a timestamped file
make save-logs DOCKER="sudo docker"

# 2. Analyze the saved log file to produce a CSV
make analyze LOG=logs/<file>.log
```

The `save-logs` target writes the log to `logs/pico-primary-<timestamp>.log`. The `analyze` target runs `scripts/analyze_log.py` on the specified log file and writes a CSV alongside it (e.g., `logs/pico-primary-<timestamp>.csv`) with columns: `tag, kind, block, idx, status, cycles, e2e_s, log_file`.

---

## 8. Managing Containers

```bash
# Stop and remove all containers
make down DOCKER="sudo docker"

# Check GPU utilization
nvidia-smi -l 1
```

---

## Troubleshooting

### Network Connectivity Issues
- Confirm the required ports are open: **50052**, **50053**, **50054**
- Check firewall rules on both machines
