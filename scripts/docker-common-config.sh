#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Configuration Loader
# =============================================================================
# This script loads configuration from YAML config file or falls back to
# hardcoded values for backward compatibility.
# =============================================================================

# Try to load configuration from YAML if available
load_yaml_config() {
    local config_file="${1:-${SCRIPT_DIR}/../config.yaml}"
    
    # Check if config file exists and yq is available
    if [[ -f "$config_file" ]] && command -v yq &> /dev/null; then
        # Parse and export config values
        eval "$(yq eval '
            "export AGG_HOST=\"" + .aggregator.host + "\"",
            "export AGG_USER=\"" + .aggregator.user + "\"",
            "export AGG_REMOTE_DIR=\"" + .aggregator.remote_dir + "\"",
            "export PERF_DATA_DIR=\"" + .paths.perf_data_dir + "\"",
            "export PROGRAM_CACHE_FILE=\"" + .paths.program_cache_file + "\"",
            "export CONTAINER_DATA_MOUNT=\"" + .paths.container_data_mount + "\"",
            "export CONTAINER_CACHE_MOUNT=\"" + .paths.container_cache_mount + "\"",
            "export LOGS_DIR=\"" + .paths.logs_dir + "\"",
            "export DOCKER_PREFIX=\"" + .docker.prefix + "\"",
            "export CPUSET_CPUS=\"" + .numa.cpuset_cpus + "\"",
            "export CPUSET_MEMS=\"" + .numa.cpuset_mems + "\"",
            "export SSH_CONNECT_TIMEOUT=\"" + (.ssh.connect_timeout | tostring) + "\"",
            "export SSH_CONTROL_PERSIST=\"" + .ssh.control_persist + "\"",
            "export SSH_MAX_RETRIES=\"" + (.ssh.max_retries | tostring) + "\"",
            "export SSH_RETRY_DELAY=\"" + (.ssh.retry_delay | tostring) + "\"",
            "export WORKER_OPERATION_DELAY=\"" + (.ssh.worker_operation_delay | tostring) + "\"",
            "export STOP_MAX_RETRIES=\"" + (.container_management.stop_max_retries | tostring) + "\"",
            "export STOP_RETRY_DELAY=\"" + (.container_management.stop_retry_delay | tostring) + "\"",
            "export AGG_STARTUP_WAIT=\"" + (.container_management.aggregator_startup_wait | tostring) + "\"",
            "export RESTART_WAIT_TIME=\"" + (.container_management.restart_wait_time | tostring) + "\"",
            "export TIMESTAMP_FORMAT=\"" + .performance.timestamp_format + "\"",
            "export LOG_DATE_FORMAT=\"" + .performance.log_date_format + "\""
        ' "$config_file" 2>/dev/null)"
        
        # Load workers array from YAML
        if command -v yq &> /dev/null 2>&1; then
            # Create workers array from YAML  
            local workers_data
            workers_data=$(yq eval '.workers[] | .host + " " + .user + " " + .worker_id + " " + (.index | tostring) + " " + .remote_dir' "$config_file" 2>/dev/null)
            
            if [[ -n "$workers_data" ]]; then
                # Convert to array
                WORKERS=()
                while IFS= read -r worker_spec; do
                    WORKERS+=("$worker_spec")
                done <<< "$workers_data"
                
                return 0  # Successfully loaded YAML config
            fi
        fi
    fi
    
    return 1  # Failed to load YAML config, use defaults
}

# Initialize configuration
init_config() {
    # Try to load from YAML first
    if load_yaml_config "${CONFIG_FILE:-}"; then
        return 0
    fi
    
    # Fall back to generic defaults (will need customization)
    # WARNING: These are generic defaults that need to be customized for your environment
    # Create a config.yaml file using './setup.sh init' for proper configuration
    
    # --- Aggregator Configuration ---
    AGG_HOST="${AGG_HOST:-192.168.1.10}"
    AGG_USER="${AGG_USER:-ubuntu}"
    AGG_REMOTE_DIR="${AGG_REMOTE_DIR:-/home/ubuntu/brevis}"

    # --- Worker Configuration ---
    if [[ ${#WORKERS[@]} -eq 0 ]]; then
        WORKERS=(
            "192.168.1.11 ubuntu worker1 0 /home/ubuntu/brevis"
            "192.168.1.12 ubuntu worker2 1 /home/ubuntu/brevis"
            "192.168.1.13 ubuntu worker3 2 /home/ubuntu/brevis"
            "192.168.1.14 ubuntu worker4 3 /home/ubuntu/brevis"
            "192.168.1.15 ubuntu worker5 4 /home/ubuntu/brevis"
            "192.168.1.16 ubuntu worker6 5 /home/ubuntu/brevis"
            "192.168.1.17 ubuntu worker7 6 /home/ubuntu/brevis"
        )
    fi

    # --- Path Configuration ---
    PERF_DATA_DIR="${PERF_DATA_DIR:-/path/to/your/project/perf/bench_data}"
    PROGRAM_CACHE_FILE="${PROGRAM_CACHE_FILE:-/path/to/your/project/program_cache.bin}"
    CONTAINER_DATA_MOUNT="${CONTAINER_DATA_MOUNT:-/app/perf/bench_data}"
    CONTAINER_CACHE_MOUNT="${CONTAINER_CACHE_MOUNT:-/app/program_cache.bin}"
    LOGS_DIR="${LOGS_DIR:-docker-logs}"

    # --- Docker Configuration ---
    DOCKER_PREFIX="${DOCKER_PREFIX:-sudo docker}"

    # --- NUMA Configuration ---
    CPUSET_CPUS="${CPUSET_CPUS:-62-123}"
    CPUSET_MEMS="${CPUSET_MEMS:-1}"

    # --- SSH Configuration ---
    SSH_CONNECT_TIMEOUT="${SSH_CONNECT_TIMEOUT:-30}"
    SSH_CONTROL_DIR="${SSH_CONTROL_DIR:-${HOME}/.ssh/control}"
    SSH_CONTROL_PERSIST="${SSH_CONTROL_PERSIST:-10m}"
    SSH_OPTIONS="${SSH_OPTIONS:--o ConnectTimeout=${SSH_CONNECT_TIMEOUT} -o ServerAliveInterval=60 -o ServerAliveCountMax=3 -o ControlMaster=auto -o ControlPath=${SSH_CONTROL_DIR}/%r@%h:%p -o ControlPersist=${SSH_CONTROL_PERSIST}}"
    SSH_MAX_RETRIES="${SSH_MAX_RETRIES:-3}"
    SSH_RETRY_DELAY="${SSH_RETRY_DELAY:-2}"
    WORKER_OPERATION_DELAY="${WORKER_OPERATION_DELAY:-0.1}"

    # --- Container Management Settings ---
    STOP_MAX_RETRIES="${STOP_MAX_RETRIES:-5}"
    STOP_RETRY_DELAY="${STOP_RETRY_DELAY:-3}"
    AGG_STARTUP_WAIT="${AGG_STARTUP_WAIT:-5}"
    RESTART_WAIT_TIME="${RESTART_WAIT_TIME:-3}"

    # --- Logging Configuration ---
    TIMESTAMP_FORMAT="${TIMESTAMP_FORMAT:-%Y%m%d-%H%M%S}"
    LOG_DATE_FORMAT="${LOG_DATE_FORMAT:-%Y-%m-%d %H:%M:%S}"
    
    return 0
}

# Load configuration when sourced
init_config
