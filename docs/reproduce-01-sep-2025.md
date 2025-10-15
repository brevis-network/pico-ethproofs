# Reproducing Results for Proving Blocks on Sep. 01, 2025

This document describes how to reproduce the results for proving of blocks on September 1 2025, using one CPU machine and eight GPU machines each with 8 GPU cards.

**Terminology Note:** The following terminology is used throughout this document:
- **CPU machine**: Runs the pico-ethproofs server and client.
- **GPU machines**: Eight machines in total (one aggregator + seven subblock workers).
- **Aggregator machine**: One GPU machine that runs the aggregator container.
- **Subblock machines**: Seven GPU machines that run subblock worker containers.
- **Aggregator container**: Docker container running on the aggregator machine.
- **Subblock worker containers**: Docker containers running on the subblock machines.

---

## 1. System Setup

### Machines
- **1 CPU machine:**  
  Runs the `pico-ethproofs` server and client, which are responsible for fetching blocks and triggering the entire proving process.  
  Set it up as a standard [Rust](https://rust-lang.org/) development environment.

- **8 GPU machines:**  
  Run the aggregator and subblock worker containers to form a proving cluster.  
  Refer to [multi-machine-setup.md](./multi-machine-setup.md) for details on setting up these GPU machines.

### Prerequisites

#### CPU Machine (`pico-ethproofs`)
- Git installed.
- Rust development environment set up.
- SSH key pair generated for accessing GPU machines.

#### GPU Machines (1 Aggregator Machine + 7 Subblock Machines)
- Docker installed.
- GPU access enabled for Docker.
- Network connectivity configured between all machines.
- SSH access properly set up.

---

## 2. Initial Setup

### 2.1 Clone Repository

On the CPU machine (`pico-ethproofs`):

```bash
git clone https://github.com/brevis-network/pico-ethproofs.git
cd pico-ethproofs
```

### 2.2 Download Program Cache

On all GPU machines (1 aggregator machine + 7 subblock machines):

1. Download the program cache:
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/ethproofs-rel-20251015/program_cache.bin.tar.gz
```

2. Extract the archive:
```bash
tar xzf program_cache.bin.tar.gz
```

### 2.3 Download Docker Images

On the GPU machines only, download the required Docker images:

1. **On the aggregator machine:**
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/ethproofs-rel-20251015/pico-aggregator.tar.gz
```

2. **On each subblock machine:**
```bash
wget https://pico-proofs.s3.us-west-2.amazonaws.com/ethproofs-rel-20251015/pico-subblock-worker.tar.gz
```

**Note:** The aggregator Docker image is only needed on the aggregator machine, and the subblock worker Docker image is only needed on the subblock machines.

### 2.4 Prepare Performance Data Files

Copy the three files from the `data` folder in this repository to the same directory on all GPU machines (1 aggregator machine + 7 subblock machines):

- `aggregator-elf`
- `subblock-elf`
- `vk_digest.bin`

**Important:** Place all three files in the same directory. The path to this directory will be configured in the `config.yaml` file under the `paths.perf_data_dir` setting.

### 2.5 Configure SSH Access

On the CPU machine (client):

1. Generate SSH key pair (if not already present):
```bash
ssh-keygen
```

2. Add the public key to all GPU machines:
   - Locate your public key file (either `~/.ssh/id_ed25519.pub` or `~/.ssh/id_rsa.pub`)
   - Copy the contents of your public key to the `~/.ssh/authorized_keys` file on each GPU machine (aggregator + all 7 subblock machines)
   - Ensure the `~/.ssh` directory and `authorized_keys` file have proper permissions on the GPU machines

3. Test SSH connectivity to confirm connections:
```bash
ssh ubuntu@<aggregator-ip>
ssh ubuntu@<subblock1-ip>
# ... repeat for all 7 subblock machines
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

Edit `config.yaml` to configure your multi-machine setup. Most fields can be left at their default values.

#### **Mandatory Configurations** (You MUST change these)

1. **Aggregator Host Configuration**
   ```yaml
   aggregator:
     host: "192.168.1.10"  # CHANGE THIS to your aggregator machine IP
     
     # Client addresses (what workers connect to)
     orchestrator_client_addr: "http://192.168.1.10:50052"      # CHANGE THIS to match your aggregator IP
     final_aggregator_client_addr: "http://192.168.1.10:50051"  # CHANGE THIS to match your aggregator IP
     
     # Proof service configuration (points to CPU machine where pico-ethproofs server runs)
     proof_service_addr: "http://192.168.1.1:58888"  # CHANGE THIS to your CPU machine IP and port
   ```

2. **Worker Machine Configuration** (7 workers required)
   ```yaml
   workers:
     - host: "192.168.1.11"      # CHANGE THIS to your worker 1 IP
       worker_id: "worker1"       # CHANGE THIS to your preferred worker name
       index: 0                   # Keep as-is (sequential 0-6)
       
     - host: "192.168.1.12"      # CHANGE THIS to your worker 2 IP
       worker_id: "worker2"       # CHANGE THIS to your preferred worker name
       index: 1                   # Keep as-is
     
     # ... (repeat for workers 3-7 with indices 2-6)
   ```

3. **Path Configuration**
   ```yaml
   paths:
     # Directory containing aggregator-elf, subblock-elf, and vk_digest.bin
     perf_data_dir: "/path/to/your/project/perf/bench_data"  # CHANGE THIS
     
     # Program cache file location (will be created if doesn't exist)
     program_cache_file: "/path/to/your/project/program_cache.bin"  # CHANGE THIS
   ```

#### **Optional Configurations** (Leave as defaults unless needed)

The following can be left at their default values:

- **SSH Settings**: `user: "ubuntu"`, `port: 22`, `remote_dir: "/home/ubuntu/brevis"`
  - Only change if you use a different SSH user, non-standard port, or different working directory
  - **Important**: The `remote_dir` must exist on all GPU machines. Create it if needed:
    ```bash
    # On each GPU machine
    mkdir -p /home/ubuntu/brevis
    ```

- **Docker Configuration**: `docker.prefix: "sudo docker"`
  - Only change if your user is in the docker group (use `"docker"` instead)

- **NUMA Settings**: `cpuset_cpus: "62-123"`, `cpuset_mems: "1"`
  - Default works for most GPU servers. Adjust only if you know your hardware's NUMA topology

- **SSH/Container Management Settings**: All retry, timeout, and connection settings
  - Defaults are tuned for reliability across different network conditions

- **Experiment Settings**: `run_id`, `chunk_size`, `split_threshold`, etc.
  - Pre-configured for experiment reproducibility
  - `run_id`: Used for logging and tracking purposes only, does not impact performance

**Summary**: Focus on setting your machine IPs (aggregator + 7 workers), the two paths (`perf_data_dir` and `program_cache_file`), and ensure `remote_dir` exists on all GPU machines. Everything else can stay at defaults.

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
- `.env.aggregator`: Environment file for the aggregator container
- `.env.subblock`: Template for subblock worker container environment files

Expected output:
```
✓ Generated: /home/ubuntu/pico-ethproofs/.env.aggregator
✓ Generated: /home/ubuntu/pico-ethproofs/.env.subblock (template)

Next: run ./setup.sh distribute
      Purpose: distribute .env files to all machines
```


### 3.6 Distribute Configuration

Distribute environment files to all machines:

```bash
./setup.sh distribute
```

This copies the appropriate `.env` files to each machine via SSH.

---

## 4. Launch Docker Containers

### 4.1 Load Docker Images

On each GPU machine, load the downloaded Docker images:

1. **On the aggregator machine:**
```bash
gunzip -c pico-aggregator.tar.gz | sudo docker load
```

2. **On each subblock machine:**
```bash
gunzip -c pico-subblock-worker.tar.gz | sudo docker load
```

### 4.2 Start Containers

Start all containers (aggregator container and subblock worker containers):

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

On the CPU machine:

1. Create a target folder:
```bash
mkdir -p /home/ubuntu/subblocks-20250901
cd /home/ubuntu/subblocks-20250901
```

**Note:** Ensure you have sufficient disk space available.

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

export PROVING_AGG_URL="<IP:50053 of the machine running the aggregator>"
export PROVING_SUBBLOCK_URLS="<IP:50053 of the machines running subblock workers>"
# We require one aggregator and seven subblock provers. For example:
# export PROVING_AGG_URL="http://0.0.0.0:50053"
# export PROVING_SUBBLOCK_URLS="http://10.0.0.1:50053,http://10.0.0.2:50053,http://10.0.0.3:50053,http://10.0.0.4:50053,http://10.0.0.5:50053,http://10.0.0.6:50053,http://10.0.0.7:50053"
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
- Check container logs on aggregator and subblock machines if needed (using `sudo docker logs -f pico-aggregator|pico-subblock-worker`)
- Wait until all proving results are saved into `proving_report.csv` in the working directory
- The process will generate performance metrics and proof verification data

---

## 9. Managing Containers

### Stop All Containers
```bash
cd scripts
./docker-multi-control.sh stop
```

### Stop and Remove All Containers
```bash
cd scripts
./docker-multi-control.sh cleanup
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

### For More Detailed Commands and Instructions
```bash
./docker-multi-control.sh -h
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

- Ensure network connectivity between aggregator machine and subblock machines
- Docker containers must have GPU access for GPU-accelerated proving
- Adjust environment variables according to your specific setup
- The total end-to-end proving time for 7,200 blocks is approximately 13.5 hours with the machine specifications described in [multi-machine-setup.md](./multi-machine-setup.md). The actual wall-clock time will be longer when accounting for network data transfer delays
- Monitor disk space as proof data can be substantial
- Keep the `proving_report.csv` file for verification and audit purposes
