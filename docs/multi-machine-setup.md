# Multi-Machine Setup

## Machine Specifications

This section documents the exact hardware & system environment used for Pico Real-Time-Proving.

* **OS:** Ubuntu **22.04.4 LTS**, kernel **5.15.0-113-generic**
* **CPU:** 124 vCPUs powered by **AMD EPYC 9355 32-Core Processor**
* **GPUs:** **8 × NVIDIA GeForce RTX 5090 (32 GB each)**, Driver **570.153.02**, CUDA runtime **12.8**
* **System RAM:** **~925 GB** total
* **NUMA:** 2 nodes

  * Node 0: vCPUs **0–61**, GPUs **0–3**, memory ≈ **463 GB**
  * Node 1: vCPUs **62–123**, GPUs **4–7**, memory ≈ **463 GB**
* **Storage:** Single virtual disk **500 GB**

## Set up a GPU Machine

This guide walks you through setting-up a GPU Ubuntu machine for Pico Real-Time Proving.

## Install NVIDIA Drivers

First, Detect NVIDIA GPUs:
```
lspci | grep -i nvidia
```

Install the recommended driver:
```
sudo apt-get update
sudo apt-get install -y nvidia-driver-570
```

If nvidia-driver-570 doesn't work, follow [this link](https://developer.nvidia.com/datacenter-driver-downloads?target_os=Linux&target_arch=x86_64&Distribution=Ubuntu&target_version=22.04&target_type=deb_local) to install nvidia-driver-580.

Reboot, then confirm:
```
nvidia-smi
```

### Install CUDA Toolkit

CUDA Toolkit 13.* doesn't work with the current code, follow [this link](https://developer.nvidia.com/cuda-12-8-0-download-archive?target_os=Linux&target_arch=x86_64&Distribution=Ubuntu&target_version=22.04&target_type=deb_local) to install cuda-toolkit-12.8.

After installing CUDA toolkit, add these lines to `~/.bashrc` or `~/.zshrc`:
```
export PATH=/usr/local/cuda-12.8/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/lib64:$LD_LIBRARY_PATH
```

Reload bash configuration:
```
source ~/.bashrc
```

Verify nvcc is available:
```
nvcc --version
```

## Install Development Tools

```
sudo apt-get install -y build-essential cmake git pkg-config
sudo apt-get install -y libssl-dev
```

And follow [this link](https://www.rust-lang.org/tools/install) to install Rust.

## Install TCMalloc

```
sudo apt-get install google-perftools
sudo ln -s /usr/lib/x86_64-linux-gnu/libtcmalloc.so.4 /usr/lib/x86_64-linux-gnu/libtcmalloc.so
```

## Install m4
```
sudo apt-get update
sudo apt-get install -y m4
```

## Install numactl
```
sudo apt-get update
sudo apt-get install -y numactl
```

## Install protoc
```
sudo apt-get install -y unzip

wget https://github.com/protocolbuffers/protobuf/releases/download/v21.12/protoc-21.12-linux-x86_64.zip
unzip protoc-21.12-linux-x86_64.zip -d protoc21.12
sudo mv protoc21.12/bin/* /usr/local/bin/
sudo mv protoc21.12/include/* /usr/local/include/
protoc --version
```
