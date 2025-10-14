#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Common Configuration and Functions
# =============================================================================
# Shared configuration and utility functions for Docker-based multi-machine setup
# =============================================================================

set -euo pipefail

# =============================================================================
# CONSTANTS - Do not modify these (modify via environment variables instead)
# =============================================================================

# Container names (keep these fixed for Docker operations)
readonly CONTAINER_NAME_AGGREGATOR="pico-aggregator"
readonly CONTAINER_NAME_WORKER="pico-subblock-worker"

# Docker image names
readonly IMAGE_NAME_AGGREGATOR="pico-aggregator:latest"
readonly IMAGE_NAME_WORKER="pico-subblock-worker:latest"

# Environment file names
readonly ENV_FILE_AGGREGATOR=".env.aggregator"
readonly ENV_FILE_WORKER=".env.subblock"

# Chunk sizes for retry mechanism
readonly CHUNK_SIZE_NORMAL=$((1 << 22))  # 4194304
readonly CHUNK_SIZE_RETRY=$((1 << 21))   # 2097152

# =============================================================================
# CONFIGURATION LOADING
# =============================================================================
# Load configuration from YAML file or use hardcoded defaults for backward compatibility

# Determine script directory for config file path
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Configuration file path (can be overridden by environment)
CONFIG_FILE="${CONFIG_FILE:-${SCRIPT_DIR}/../config.yaml}"

# Source the configuration loader
source "${SCRIPT_DIR}/docker-common-config.sh"

# Update SSH_OPTIONS after configuration is loaded
SSH_CONTROL_DIR="${SSH_CONTROL_DIR:-${HOME}/.ssh/control}"
SSH_OPTIONS="${SSH_OPTIONS:--o ConnectTimeout=${SSH_CONNECT_TIMEOUT} -o ServerAliveInterval=60 -o ServerAliveCountMax=3 -o ControlMaster=auto -o ControlPath=${SSH_CONTROL_DIR}/%r@%h:%p -o ControlPersist=${SSH_CONTROL_PERSIST}}"

# =============================================================================
# UTILITY FUNCTIONS
# =============================================================================

# --- Logging Functions ---

log() {
    echo "[$(date +"$LOG_DATE_FORMAT")] $*"
}

error() {
    echo "[ERROR] $*" >&2
}

warn() {
    echo "[WARN] $*" >&2
}

info() {
    echo "[INFO] $*"
}

# --- SSH Helper Functions ---

# Initialize SSH connection multiplexing control directory
init_ssh_control() {
    if [[ ! -d "$SSH_CONTROL_DIR" ]]; then
        mkdir -p "$SSH_CONTROL_DIR" 2>/dev/null || true
        chmod 700 "$SSH_CONTROL_DIR" 2>/dev/null || true
    fi
}

# Establish multiplexed SSH connection to a host (optional, connections are auto-created)
establish_ssh_connection() {
    local user="$1"
    local host="$2"
    local port="${3:-22}"
    
    # Build SSH options with port
    local ssh_opts="$SSH_OPTIONS"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    # Send a simple command to establish the connection
    # The ControlMaster=auto will create the control socket
    ssh $ssh_opts "${user}@${host}" "true" 2>/dev/null || true
}

# Close multiplexed SSH connection to a host
close_ssh_connection() {
    local user="$1"
    local host="$2"
    local port="${3:-22}"
    
    # Build SSH options with port
    local ssh_opts="$SSH_OPTIONS"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    ssh $ssh_opts -O exit "${user}@${host}" 2>/dev/null || true
}

# Execute SSH command with configured options and retry logic
# Properly handles stderr and supports commands with pipes
ssh_exec() {
    local user="$1"
    local host="$2"
    local port="${3:-22}"
    shift 3
    
    local retry=0
    local max_retries="$SSH_MAX_RETRIES"
    
    # Build SSH options with port
    local ssh_opts="$SSH_OPTIONS"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    while [[ $retry -lt $max_retries ]]; do
        # Use bash -c to properly handle pipes and redirections
        # Allow stderr to pass through (it may be normal output from docker commands)
        ssh $ssh_opts "${user}@${host}" "bash -c $(printf '%q' "$*")"
        local exit_code=$?
        
        # Only retry on SSH connection failure (exit code 255)
        # All other exit codes (including 0) are returned as-is
        if [[ $exit_code -ne 255 ]]; then
            return $exit_code
        fi
        
        # SSH connection error - retry
        retry=$((retry + 1))
        
        if [[ $retry -lt $max_retries ]]; then
            warn "SSH connection failed to ${user}@${host}, retrying ($retry/$max_retries)..."
            sleep "$SSH_RETRY_DELAY"
        else
            error "SSH connection failed to ${user}@${host} after $max_retries attempts"
            return $exit_code
        fi
    done
    
    return 1
}

# Execute SSH command without retry (for operations where retry doesn't make sense)
ssh_exec_no_retry() {
    local user="$1"
    local host="$2"
    local port="${3:-22}"
    shift 3
    
    # Build SSH options with port
    local ssh_opts="$SSH_OPTIONS"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    ssh $ssh_opts "${user}@${host}" "bash -c $(printf '%q' "$*")" 2>&1
}

# Copy file via SCP with configured options and retry logic
scp_copy() {
    local src="$1"
    local user="$2"
    local host="$3"
    local dest="$4"
    local port="${5:-22}"
    
    local retry=0
    local max_retries="$SSH_MAX_RETRIES"
    
    # Build SCP options with port (SCP uses -P, not -p)
    local scp_opts="$SSH_OPTIONS"
    if [[ "$port" != "22" ]]; then
        scp_opts="$scp_opts -P $port"
    fi
    
    while [[ $retry -lt $max_retries ]]; do
        if scp $scp_opts "$src" "${user}@${host}:${dest}" 2>&1; then
            return 0
        else
            local exit_code=$?
            retry=$((retry + 1))
            
            if [[ $retry -lt $max_retries ]]; then
                warn "SCP failed to ${user}@${host}, retrying ($retry/$max_retries)..."
                sleep "$SSH_RETRY_DELAY"
            else
                error "SCP failed to ${user}@${host} after $max_retries attempts"
                return $exit_code
            fi
        fi
    done
    
    return 1
}

# --- Worker Management Functions ---

# Get worker array element by index
get_worker() {
    local idx="$1"
    if [[ $idx -lt ${#WORKERS[@]} ]]; then
        echo "${WORKERS[$idx]}"
        return 0
    fi
    return 1
}

# Parse worker spec: "HOST USER PORT WORKER_ID INDEX REMOTE_DIR"
parse_worker_spec() {
    local spec="$1"
    read -r host user port wid idx remote_dir <<< "$spec"
    echo "$host" "$user" "$port" "$wid" "$idx" "$remote_dir"
}

# Build expected workers and indices CSV lists for aggregator configuration
build_worker_lists() {
    local worker_ids=()
    local indices=()
    
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        worker_ids+=("$wid")
        indices+=("$idx")
    done
    
    local workers_csv=$(IFS=,; echo "${worker_ids[*]}")
    local indices_csv=$(IFS=,; echo "${indices[*]}")
    
    echo "$workers_csv" "$indices_csv"
}

# Apply delay between worker operations if configured
apply_worker_delay() {
    if [[ "${WORKER_OPERATION_DELAY}" != "0" ]] && [[ -n "${WORKER_OPERATION_DELAY}" ]]; then
        sleep "$WORKER_OPERATION_DELAY"
    fi
}

# --- Configuration Validation ---

validate_config() {
    local errors=0
    
    if [[ -z "$AGG_HOST" ]]; then
        error "AGG_HOST is not set"
        ((errors++))
    fi
    
    if [[ -z "$AGG_USER" ]]; then
        error "AGG_USER is not set"
        ((errors++))
    fi
    
    if [[ ${#WORKERS[@]} -eq 0 ]]; then
        error "No workers configured"
        ((errors++))
    fi
    
    if [[ -z "$PERF_DATA_DIR" ]]; then
        error "PERF_DATA_DIR is not set"
        ((errors++))
    fi
    
    return $errors
}

# =============================================================================
# CONTAINER MANAGEMENT FUNCTIONS
# =============================================================================

# --- Low-Level Container Operations ---

# Check if container exists (running or stopped)
container_exists() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX ps -a --format '{{.Names}}' | grep -q '^${container_name}$' 2>/dev/null"
}

# Check if container is running
is_container_running() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX ps --format '{{.Names}}' | grep -q '^${container_name}$' 2>/dev/null"
}

# Stop a Docker container with retry logic to handle zombie processes
stop_container_with_retry() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    local max_retries="${5:-$STOP_MAX_RETRIES}"
    local retry_delay="${6:-$STOP_RETRY_DELAY}"
    
    log "Stopping container $container_name on ${user}@${host}..."
    
    local retry=0
    while [[ $retry -lt $max_retries ]]; do
        # Try to stop the container
        if ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX stop $container_name 2>&1" | grep -q "zombie"; then
            retry=$((retry + 1))
            if [[ $retry -lt $max_retries ]]; then
                warn "Container $container_name is zombie, retrying ($retry/$max_retries)..."
                sleep "$retry_delay"
            else
                error "Failed to stop $container_name after $max_retries attempts (zombie process)"
                # Try force kill as last resort
                log "Attempting force kill on $container_name..."
                ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX kill $container_name 2>/dev/null || true"
                sleep 2
                return 1
            fi
        else
            # Successfully stopped or container doesn't exist
            log "Container $container_name stopped successfully"
            return 0
        fi
    done
    
    return 1
}

# Save container logs to a file
save_container_logs() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local log_file="$4"
    local port="${5:-22}"
    
    log "Saving logs from $container_name on ${user}@${host} to $log_file..."
    
    # Create logs directory if it doesn't exist
    ssh_exec "$user" "$host" "$port" "mkdir -p \$(dirname '$log_file')"
    
    # Check if container exists first
    if ! container_exists "$host" "$user" "$container_name" "$port"; then
        log "Container $container_name does not exist on ${user}@${host}, skipping log save"
        return 0
    fi
    
    # Save logs
    if ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX logs $container_name &> '$log_file' 2>&1"; then
        log "Logs saved to $log_file"
        return 0
    else
        # Check if the error is because container doesn't exist (race condition)
        if ! container_exists "$host" "$user" "$container_name" "$port"; then
            log "Container $container_name was removed during log save, skipping"
            return 0
        else
            error "Failed to save logs from $container_name"
            return 1
        fi
    fi
}

# Stop and remove a container
stop_and_remove_container() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    
    stop_container_with_retry "$host" "$user" "$container_name" "$port"
    
    log "Removing container $container_name on ${user}@${host}..."
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rm $container_name 2>/dev/null || true"
}

# Force remove a container (running or stopped)
force_remove_container() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    
    log "Force removing container $container_name on ${user}@${host}..."
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rm -f $container_name 2>/dev/null || true"
}

# Check if container is completely gone (doesn't exist at all)
is_container_gone() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    
    # Returns 0 if container is gone, 1 if it still exists
    if ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^${container_name}$'"; then
        return 1  # Container still exists
    else
        return 0  # Container is gone
    fi
}

# Force kill and remove a container with verification (immediate termination)
force_kill_container() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local port="${4:-22}"
    local max_retries="${5:-3}"
    local retry_delay="${6:-2}"
    
    log "Force killing container $container_name on ${user}@${host}..."
    
    local retry=0
    while [[ $retry -lt $max_retries ]]; do
        # Kill the container immediately
        ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX kill $container_name 2>/dev/null || true"
        
        # Wait a moment for kill to take effect
        sleep 1
        
        # Force remove
        ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rm -f $container_name 2>/dev/null || true"
        
        # Wait a moment for removal to complete
        sleep 1
        
        # Verify container is gone
        if is_container_gone "$host" "$user" "$container_name" "$port"; then
            log "Container $container_name successfully removed"
            return 0
        else
            retry=$((retry + 1))
            if [[ $retry -lt $max_retries ]]; then
                warn "Container $container_name still exists, retrying ($retry/$max_retries)..."
                sleep "$retry_delay"
            else
                error "Failed to remove $container_name after $max_retries attempts"
                return 1
            fi
        fi
    done
    
    return 1
}

# Force kill and remove all containers matching a pattern
force_kill_all_containers() {
    local host="$1"
    local user="$2"
    local pattern="$3"
    local port="${4:-22}"
    
    log "Force killing all containers matching '$pattern' on ${user}@${host}..."
    
    # Kill all matching containers
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX ps -a --filter name=$pattern -q | xargs -r $DOCKER_PREFIX kill 2>/dev/null || true"
    
    # Wait a moment
    sleep 1
    
    # Force remove all matching containers
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX ps -a --filter name=$pattern -q | xargs -r $DOCKER_PREFIX rm -f 2>/dev/null || true"
}

# --- Docker Run Command Builder ---

# Build common Docker run options
build_docker_run_opts() {
    local container_name="$1"
    local env_file="$2"
    
    echo "-d \
        --name $container_name \
        --gpus all \
        --network host \
        --ipc=host \
        --cap-add=SYS_NICE \
        --ulimit memlock=-1:-1 \
        --env-file $env_file \
        -v ${PERF_DATA_DIR}:${CONTAINER_DATA_MOUNT}:ro \
        -v ${PROGRAM_CACHE_FILE}:${CONTAINER_CACHE_MOUNT}:rw \
        --cpuset-cpus='${CPUSET_CPUS}' \
        --cpuset-mems='${CPUSET_MEMS}'"
}

# =============================================================================
# AGGREGATOR FUNCTIONS
# =============================================================================

# Stop aggregator container
stop_aggregator() {
    local save_logs="${1:-true}"
    
    if [[ "$save_logs" == "true" ]]; then
        local timestamp=$(date +"$TIMESTAMP_FORMAT")
        local log_file="${AGG_REMOTE_DIR}/${LOGS_DIR}/aggregator-${timestamp}.log"
        save_container_logs "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$log_file" "$AGG_PORT" || true
    fi
    
    stop_and_remove_container "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$AGG_PORT"
}

# Start aggregator container
start_aggregator() {
    local env_file="${1:-$ENV_FILE_AGGREGATOR}"
    
    log "Starting aggregator on ${AGG_USER}@${AGG_HOST}..."
    
    local docker_opts=$(build_docker_run_opts "$CONTAINER_NAME_AGGREGATOR" "$env_file")
    
    ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "cd '$AGG_REMOTE_DIR' && \
        $DOCKER_PREFIX run $docker_opts $IMAGE_NAME_AGGREGATOR"
    
    log "Aggregator started successfully"
}

# Get aggregator status
# get_aggregator_status() {
#     log "Aggregator status on ${AGG_USER}@${AGG_HOST}:"
    
#     # Check if container is running
#     # if ssh_exec "$AGG_USER" "$AGG_HOST" "$DOCKER_PREFIX ps --format '{{.Names}}' | grep -q '^${CONTAINER_NAME_AGGREGATOR}$' 2>/dev/null"; then
#     #     ssh_exec "$AGG_USER" "$AGG_HOST" "$DOCKER_PREFIX ps | grep $CONTAINER_NAME_AGGREGATOR"
#     # # Check if container exists but is stopped
#     # elif ssh_exec "$AGG_USER" "$AGG_HOST" "$DOCKER_PREFIX ps -a --format '{{.Names}}' | grep -q '^${CONTAINER_NAME_AGGREGATOR}$' 2>/dev/null"; then
#     #     echo "  Status: STOPPED (container exists but not running)"
#     #     ssh_exec "$AGG_USER" "$AGG_HOST" "$DOCKER_PREFIX ps -a | grep $CONTAINER_NAME_AGGREGATOR"
#     # else
#     #     echo "  Status: DOES NOT EXIST"
#     # fi

#     if ssh_exec "$AGG_USER" "$AGG_HOST" "sudo docker ps --format '{{.Names}}' | grep -q '^pico-aggregator\$'"; then
#         ssh_exec "$AGG_USER" "$AGG_HOST" "sudo docker ps | grep pico-aggregator"
#     elif ssh_exec "$AGG_USER" "$AGG_HOST" "sudo docker ps -a --format '{{.Names}}' | grep -q '^pico-aggregator\$'"; then
#         echo "  Status: STOPPED (container exists but not running)"
#         ssh_exec "$AGG_USER" "$AGG_HOST" "sudo docker ps -a | grep pico-aggregator"
#     else
#         echo "  Status: DOES NOT EXIST"
#     fi
# }

# Get aggregator status
get_aggregator_status() {
    log "Aggregator status on ${AGG_USER}@${AGG_HOST}:"
    
    # Step 1: Get running containers
    local running_containers
    running_containers=$(ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "sudo docker ps --format '{{.Names}}'")
    
    if [[ $? -ne 0 ]]; then
        error "Failed to connect to aggregator at ${AGG_USER}@${AGG_HOST}"
        echo "  Status: CONNECTION FAILED"
        return 1
    fi
    
    echo "  [✓] SSH connected"
    
    # Step 2: Check if container is running (locally)
    if echo "$running_containers" | grep -q '^pico-aggregator$'; then
        echo "  Status: RUNNING"
        ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "sudo docker ps | grep pico-aggregator"
        return 0
    fi
    
    # Step 3: Check if container exists but stopped
    local all_containers
    all_containers=$(ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "sudo docker ps -a --format '{{.Names}}'")
    
    if [[ $? -ne 0 ]]; then
        error "Failed to connect to aggregator at ${AGG_USER}@${AGG_HOST}"
        echo "  Status: CONNECTION FAILED"
        return 1
    fi
    
    if echo "$all_containers" | grep -q '^pico-aggregator$'; then
        echo "  Status: STOPPED (container exists but not running)"
        ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "sudo docker ps -a | grep pico-aggregator"
        return 0
    fi
    
    echo "  Status: DOES NOT EXIST"
    return 0
}


# Cleanup aggregator container
cleanup_aggregator() {
    log "Cleaning up aggregator container on ${AGG_USER}@${AGG_HOST}..."
    force_kill_container "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$AGG_PORT"
}

# Force kill aggregator container (immediate termination)
force_kill_aggregator() {
    log "Force killing aggregator container on ${AGG_USER}@${AGG_HOST}..."
    force_kill_container "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$AGG_PORT"
    return $?
}

# =============================================================================
# WORKER FUNCTIONS
# =============================================================================

# Stop a single worker container
stop_worker() {
    local host="$1"
    local user="$2"
    local port="$3"
    local wid="$4"
    local save_logs="${5:-false}"
    local remote_dir="$6"
    
    if [[ "$save_logs" == "true" ]]; then
        local timestamp=$(date +"$TIMESTAMP_FORMAT")
        local log_file="${remote_dir}/${LOGS_DIR}/subblock-${wid}-${timestamp}.log"
        save_container_logs "$host" "$user" "$CONTAINER_NAME_WORKER" "$log_file" "$port" || true
    fi
    
    stop_and_remove_container "$host" "$user" "$CONTAINER_NAME_WORKER" "$port"
}

# Start a single worker container
start_worker() {
    local host="$1"
    local user="$2"
    local port="$3"
    local wid="$4"
    local remote_dir="$5"
    local env_file="${6:-$ENV_FILE_WORKER}"
    
    log "Starting worker $wid on ${user}@${host}..."
    
    local docker_opts=$(build_docker_run_opts "$CONTAINER_NAME_WORKER" "$env_file")
    
    ssh_exec "$user" "$host" "$port" "cd '$remote_dir' && \
        $DOCKER_PREFIX run $docker_opts $IMAGE_NAME_WORKER"
    
    log "Worker $wid started successfully"
}

# Cleanup a single worker container
cleanup_worker() {
    local host="$1"
    local user="$2"
    local port="$3"
    local wid="$4"
    
    log "Cleaning up worker $wid on ${user}@${host}..."
    force_kill_container "$host" "$user" "$CONTAINER_NAME_WORKER" "$port"
}

# Stop all worker containers
stop_all_workers() {
    local save_logs="${1:-false}"
    
    log "Stopping all ${#WORKERS[@]} workers..."
    
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        stop_worker "$host" "$user" "$port" "$wid" "$save_logs" "$remote_dir"
        apply_worker_delay
    done
    
    log "All workers stopped"
}

# Start all worker containers
start_all_workers() {
    log "Starting all ${#WORKERS[@]} workers..."
    
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        start_worker "$host" "$user" "$port" "$wid" "$remote_dir"
        apply_worker_delay
    done
    
    log "All workers started"
}

# Get status of all worker containers
# get_all_worker_status() {
#     for worker_spec in "${WORKERS[@]}"; do
#         read -r host user wid idx remote_dir <<< "$worker_spec"
#         log "Worker $wid status on ${user}@${host}:"
        
#         # Check if container is running
#         if ssh_exec "$user" "$host" "$DOCKER_PREFIX ps --format '{{.Names}}' | grep -q '^${CONTAINER_NAME_WORKER}$' 2>/dev/null"; then
#             ssh_exec "$user" "$host" "$DOCKER_PREFIX ps | grep $CONTAINER_NAME_WORKER"
#         # Check if container exists but is stopped
#         elif ssh_exec "$user" "$host" "$DOCKER_PREFIX ps -a --format '{{.Names}}' | grep -q '^${CONTAINER_NAME_WORKER}$' 2>/dev/null"; then
#             echo "  Status: STOPPED (container exists but not running)"
#             ssh_exec "$user" "$host" "$DOCKER_PREFIX ps -a | grep $CONTAINER_NAME_WORKER"
#         else
#             echo "  Status: DOES NOT EXIST"
#         fi
#     done
# }

# Get status of all worker containers
get_all_worker_status() {
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        log "Worker $wid status on ${user}@${host}:"
        
        # Step 1: Get running containers
        local running_containers
        running_containers=$(ssh_exec "$user" "$host" "$port" "sudo docker ps --format '{{.Names}}'")
        
        if [[ $? -ne 0 ]]; then
            error "Failed to connect to worker $wid at ${user}@${host}"
            echo "  Status: CONNECTION FAILED"
            continue
        fi
        
        echo "  [✓] SSH connected"
        
        # Step 2: Check if container is running (locally)
        if echo "$running_containers" | grep -q '^pico-subblock-worker$'; then
            echo "  Status: RUNNING"
            ssh_exec "$user" "$host" "$port" "sudo docker ps | grep pico-subblock-worker"
            continue
        fi
        
        # Step 3: Check if container exists but stopped
        local all_containers
        all_containers=$(ssh_exec "$user" "$host" "$port" "sudo docker ps -a --format '{{.Names}}'")
        
        if [[ $? -ne 0 ]]; then
            error "Failed to connect to worker $wid at ${user}@${host}"
            echo "  Status: CONNECTION FAILED"
            continue
        fi
        
        if echo "$all_containers" | grep -q '^pico-subblock-worker$'; then
            echo "  Status: STOPPED (container exists but not running)"
            ssh_exec "$user" "$host" "$port" "sudo docker ps -a | grep pico-subblock-worker"
            continue
        fi
        
        echo "  Status: DOES NOT EXIST"
    done
}

# Cleanup all worker containers
cleanup_all_workers() {
    log "Cleaning up all ${#WORKERS[@]} workers..."
    
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        cleanup_worker "$host" "$user" "$port" "$wid"
        apply_worker_delay
    done
    
    log "All workers cleaned up"
}

# Force kill a single worker container (immediate termination)
force_kill_worker() {
    local host="$1"
    local user="$2"
    local port="$3"
    local wid="$4"
    
    log "Force killing worker $wid on ${user}@${host}..."
    force_kill_container "$host" "$user" "$CONTAINER_NAME_WORKER" "$port"
    return $?
}

# Force kill all worker containers (immediate termination)
force_kill_all_workers() {
    log "Force killing all ${#WORKERS[@]} workers..."
    
    local failures=0
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        if ! force_kill_worker "$host" "$user" "$port" "$wid"; then
            ((failures++))
        fi
        apply_worker_delay
    done
    
    if [[ $failures -eq 0 ]]; then
        log "All workers force killed successfully"
        return 0
    else
        error "$failures worker(s) failed to be removed"
        return 1
    fi
}

# =============================================================================
# COMBINED FUNCTIONS
# =============================================================================

# Stop all containers (aggregator + workers)
stop_all() {
    local save_logs="${1:-true}"
    
    log "=== Stopping all containers ==="
    stop_aggregator "$save_logs"
    stop_all_workers "$save_logs"
    log "=== All containers stopped ==="
}

# Start all containers (aggregator + workers)
start_all() {
    log "=== Starting all containers ==="
    start_aggregator
    log "Waiting ${AGG_STARTUP_WAIT}s for aggregator to start..."
    sleep "$AGG_STARTUP_WAIT"
    start_all_workers
    log "=== All containers started ==="
}

# Restart all containers
restart_all() {
    log "=== Restarting all containers ==="
    stop_all true
    log "Waiting ${RESTART_WAIT_TIME}s before restart..."
    sleep "$RESTART_WAIT_TIME"
    start_all
    log "=== All containers restarted ==="
}

# Show status of all containers
show_all_status() {
    log "=== Container Status ==="
    get_aggregator_status
    echo ""
    get_all_worker_status
}

# Cleanup all containers (force remove without logs)
cleanup_all() {
    log "=== Cleaning up all containers ==="
    cleanup_aggregator
    cleanup_all_workers
    # Wait a moment to ensure cleanup completes
    sleep 2
    log "=== All containers cleaned up ==="
}

# Verify all containers are completely removed
verify_all_containers_gone() {
    log "Verifying all containers are removed..."
    
    local failures=0
    
    # Check aggregator
    if ! is_container_gone "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$AGG_PORT"; then
        error "Aggregator container still exists on ${AGG_USER}@${AGG_HOST}"
        ((failures++))
    fi
    
    # Check workers
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        if ! is_container_gone "$host" "$user" "$CONTAINER_NAME_WORKER" "$port"; then
            error "Worker $wid container still exists on ${user}@${host}"
            ((failures++))
        fi
    done
    
    if [[ $failures -eq 0 ]]; then
        log "All containers verified as removed"
        return 0
    else
        error "$failures container(s) still exist"
        return 1
    fi
}

# Force kill all containers (immediate termination, no logs)
force_kill_all() {
    log "=== Force killing all containers ==="
    
    local agg_result=0
    local workers_result=0
    
    force_kill_aggregator || agg_result=$?
    force_kill_all_workers || workers_result=$?
    
    # Wait a moment to ensure kill completes
    sleep 2
    
    if [[ $agg_result -eq 0 ]] && [[ $workers_result -eq 0 ]]; then
        log "=== All containers force killed successfully ==="
        return 0
    else
        error "=== Some containers failed to be killed ==="
        return 1
    fi
}

# =============================================================================
# SSH CONNECTION MANAGEMENT
# =============================================================================

# Initialize SSH control directory and establish connections to all hosts
init_all_ssh_connections() {
    # Create SSH control directory
    init_ssh_control
    
    # Establish connection to aggregator (in background)
    establish_ssh_connection "$AGG_USER" "$AGG_HOST" "$AGG_PORT" &
    
    # Establish connections to all workers (in background)
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        establish_ssh_connection "$user" "$host" "$port" &
    done
    
    # Wait for all background connections to complete
    wait
}

# Close all SSH connections
close_all_ssh_connections() {
    # Close aggregator connection
    close_ssh_connection "$AGG_USER" "$AGG_HOST" "$AGG_PORT"
    
    # Close worker connections
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        close_ssh_connection "$user" "$host" "$port"
    done
}

# =============================================================================
# FUNCTION EXPORTS
# =============================================================================
# Export functions for use in other scripts

# Logging functions
export -f log error warn info

# SSH helper functions
export -f ssh_exec ssh_exec_no_retry scp_copy
export -f init_ssh_control establish_ssh_connection close_ssh_connection
export -f init_all_ssh_connections close_all_ssh_connections

# Worker management functions
export -f get_worker parse_worker_spec build_worker_lists apply_worker_delay

# Configuration validation
export -f validate_config

# Container operations
export -f container_exists is_container_running is_container_gone
export -f stop_container_with_retry save_container_logs
export -f stop_and_remove_container force_remove_container
export -f force_kill_container force_kill_all_containers
export -f build_docker_run_opts verify_all_containers_gone

# Aggregator functions
export -f stop_aggregator start_aggregator get_aggregator_status
export -f cleanup_aggregator force_kill_aggregator

# Worker functions
export -f stop_worker start_worker cleanup_worker force_kill_worker
export -f stop_all_workers start_all_workers get_all_worker_status
export -f cleanup_all_workers force_kill_all_workers

# Combined functions
export -f stop_all start_all restart_all show_all_status
export -f cleanup_all force_kill_all

