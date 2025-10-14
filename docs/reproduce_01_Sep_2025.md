# Reproducing Proving Blocks of Sep 01 2025

This document describes how to reproduce proving the blocks of Sep 01 2025 by using 1 CPU machine and 8 GPU machines.

---

## 1. System Setup

### Machines
- **1 CPU machine:** run a pico-ethproofs server and a client for fetching the blocks and triggering the whole proving process, setup a normal [Rust](https://rust-lang.org/) development machine.
- **8 GPU machines:** run the Aggregator and Subblock docker containers as a proving cluster, reference [multi-machine-setup.md](./multi-machine-setup.md) for these GPU machine setup.

### Prerequisites

#### All Machines
- Docker installed and configured
- GPU access enabled for Docker (GPU machines only)
- Network connectivity between all machines
- SSH access configured

#### CPU Machine (Client)
- Git installed
- Rust development environment
- SSH key pair generated for accessing worker machines

---

## 2. Initial Setup

### 2.1 Clone Repository

On the CPU machine (client):

```bash
git clone https://github.com/brevis-network/pico-ethproofs.git
cd pico-ethproofs
```

### 2.2 Download Program Cache

On all machines (aggregator + workers):

1. Download the program cache:
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/ethproofs-rel-20251015/program_cache.bin.tar.gz
```

2. Extract the archive:
```bash
tar xzf program_cache.bin.tar.gz
```

### 2.2 Prepare Performance Data Files

Put all three files from `pico-ethproof/data` into a single folder on all workers (aggregator + subblock machines).

### 2.3 Configure SSH Access

On the CPU machine (client):

1. Generate SSH key pair (if not already present):
```bash
ssh-keygen
```

2. Add the public key to all worker machines:
```bash
ssh-copy-id ubuntu@<aggregator-ip>
ssh-copy-id ubuntu@<worker1-ip>
# ... repeat for all workers
```

3. Test SSH connectivity to confirm connections:
```bash
ssh ubuntu@<aggregator-ip>
ssh ubuntu@<worker1-ip>
# ... repeat for all workers
```

---

## 3. Configure Multi-Machine Setup

### 3.1 Initialize Configuration

On the CPU machine:

```bash
cd scripts
./setup.sh init
```

### 3.2 Modify Configuration

Edit `config.yaml` to configure:
- SSH connection details for all machines
- `perf_data_dir`: Path to performance data directory
- `program_cache_file`: Path to program cache file
- Worker machine IP addresses and ports
- NUMA settings (if applicable)

### 3.3 Validate Configuration

```bash
./setup.sh validate
```

Expected output:
```
✓ Configuration validation passed
ℹ Config file: /home/ubuntu/pico-ethproofs/config.yaml
ℹ Aggregator: <aggregator-ip>
ℹ Workers: 7
```

### 3.4 System Checks

Run comprehensive system checks:

```bash
./setup.sh check-all
```

This command verifies:
- SSH connectivity to all machines
- Docker installation on all machines
- GPU availability on all machines
- Required paths exist on all machines

Expected output:
```
✓ SSH connectivity check passed
✓ Docker installation check passed
✓ GPU availability check passed
✓ Required paths check passed
✓ All checks passed
```

### 3.5 Generate Environment Files

Generate `.env` files from configuration:

```bash
./setup.sh generate-env
```

This creates:
- `.env.aggregator`: Environment file for the aggregator machine
- `.env.subblock`: Template for subblock worker environment files

Expected output:
```
✓ Generated: /home/ubuntu/pico-ethproofs/.env.aggregator
✓ Generated: /home/ubuntu/pico-ethproofs/.env.subblock (template)

Next: run ./setup.sh validate
      Purpose: validate the generated .env files
```

### 3.6 Validate Generated Environment Files

Validate the generated `.env` files:

```bash
./setup.sh validate
```

This ensures all generated environment files are correctly formatted and contain required variables.

### 3.7 Distribute Configuration

Distribute environment files to all machines:

```bash
./setup.sh distribute
```

This copies the appropriate `.env` files to each machine via SSH.

---

## 4. Launch Docker Containers

### 4.1 Deploy Docker Images

Deploy Docker images from the aggregator machine to all machines:

```bash
cd scripts
./docker-multi-control.sh deploy
```

### 4.2 Start Containers

Start all containers (aggregator and workers):

```bash
./docker-multi-control.sh start
```

Expected output:
```
=== Docker Multi-Machine Start ===
Starting aggregator on ubuntu@<aggregator-ip>...
Aggregator started successfully
Waiting 5s for aggregator to start...
Starting all 7 workers...
Worker worker1 started successfully
Worker worker2 started successfully
...
=== All containers started ===
```

### 4.3 Verify Container Status

Check that all containers are running:

```bash
./docker-multi-control.sh status
```

All containers should show status: `RUNNING`

---

## 5. Download Subblock Data

On the CPU instance:

1. Create a target folder:
```bash
mkdir -p /home/ubuntu/subblocks-20250901
cd /home/ubuntu/subblocks-20250901
```

2. Download all block inputs:
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

## 6. Start `pico-ethproofs` Server

### 6.1 Set Environment Variables

```bash
export RUST_LOG=info
export RPC_HTTP_URL="YOUR_RPC_HTTP_URL"
export RPC_WS_URL="YOUR_RPC_WS_URL"

export PROVING_AGG_URL="<AGG_URL from multi-machine setup>"
export PROVING_SUBBLOCK_URLS="<SUBBLOCK_URL1,SUBBLOCK_URL2,... from multi-machine setup>"
```

**Note:** Replace placeholder values with your actual RPC endpoints and the URLs from your multi-machine setup configuration.

### 6.2 Run Server

```bash
cargo run -r --bin eth-proofs -- \
  --input-load-dir /home/ubuntu/subblocks-20250901
```

---

## 7. Start `ethproofs-client`

Open another terminal on the CPU machine:

```bash
cargo run -r --bin reproduce-block-by-number -- \
  --start-block-num 23264565 \
  --count 7200
```

This will process 7,200 blocks starting from block 23264565.

---

## 8. Verify Logs and Output

Monitor the proving process:

- Logs will show progress for each proving task
- Check container logs on worker machines if needed
- Wait until all proving results are saved into `proving_report.csv` in the working directory
- The process will generate performance metrics and proof verification data

---

## 9. Managing Containers

### Stop All Containers
```bash
cd scripts
./docker-multi-control.sh stop
```

### Restart Containers
```bash
./docker-multi-control.sh restart
```

### Check Container Status
```bash
./docker-multi-control.sh status
```

### View Container Logs
```bash
./docker-multi-control.sh logs [aggregator|worker1|worker2|...]
```

---

## Troubleshooting

### SSH Connection Issues
- Verify SSH keys are properly distributed to all machines
- Confirm firewall rules allow SSH connections
- Test manual SSH connection to each machine

### Docker Container Failures
- Check Docker logs: `docker logs <container-name>`
- Verify GPU access: `nvidia-smi` on GPU machines
- Ensure all required files and directories exist
- Validate environment variables in `.env` files

### Performance Issues
- Monitor GPU utilization: `nvidia-smi -l 1`
- Check system resources: CPU, memory, disk I/O
- Verify network latency between machines
- Review NUMA settings if applicable

### Proving Errors
- Check RPC endpoint connectivity and rate limits
- Verify subblock data integrity
- Review server logs for specific error messages
- Ensure program cache is properly loaded

---

## Notes

- Ensure network connectivity between aggregator and subblock machines
- Docker containers must have GPU access for GPU-accelerated proving
- Adjust environment variables according to your specific setup
- The total end-to-end proving time for 7,200 blocks is approximately 13.5 hours with the machine specifications described in [multi-machine-setup.md](./multi-machine-setup.md). The actual wall-clock time will be longer when accounting for network data transfer delays
- Monitor disk space as proof data can be substantial
- Keep the `proving_report.csv` file for verification and audit purposes