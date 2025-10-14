#!/usr/bin/env bash
# =============================================================================
# Multi-Machine Docker Setup Script  
# =============================================================================
# This script helps users set up the multi-machine Docker environment.
# It validates SSH connections, checks requirements, and configures the system.
# =============================================================================

set -euo pipefail

# Determine script directory and parent directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARENT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_TEMPLATE="${SCRIPT_DIR}/config.template.yaml"
CONFIG_FILE="${PARENT_DIR}/config.yaml"
HOSTS_DIR="${PARENT_DIR}/hosts"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'  
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log() {
    echo -e "[$(date +'%H:%M:%S')] $*"
}

success() {
    echo -e "[$(date +'%H:%M:%S')] ${GREEN}✓${NC} $*"
}

warning() {
    echo -e "[$(date +'%H:%M:%S')] ${YELLOW}⚠${NC} $*" >&2
}

error() {
    echo -e "[$(date +'%H:%M:%S')] ${RED}✗${NC} $*" >&2
}

info() {
    echo -e "[$(date +'%H:%M:%S')] ${BLUE}ℹ${NC} $*"
}

next_step() {
    echo -e "\n${GREEN}Next:${NC} $1"
    if [[ $# -gt 1 ]]; then
        echo -e "      ${BLUE}Purpose:${NC} $2"
    fi
}

usage() {
    cat <<EOF
Usage: $0 COMMAND [OPTIONS]

Multi-Machine Docker Setup and Management

Commands:
    init                Initialize configuration from template
    validate            Validate current configuration and environment  
    check-ssh           Check SSH connectivity to all machines
    check-docker        Check Docker installation on all machines
    check-gpu           Check GPU availability on all machines
    check-paths         Check required paths exist on all machines
    check-all           Run all validation checks
    generate-env        Generate .env files from existing config.yaml (does NOT touch config.yaml)
    generate-config     Generate config.yaml from template and host overrides (use with caution)
    distribute          Distribute .env files to all machines
    setup-ssh           Setup SSH key authentication (interactive)
    clean              Clean up generated files

Options:
    --verbose, -v       Show detailed output
    --dry-run, -n      Show what would be done without executing
    --force, -f        Force operation (skip confirmations)
    --config FILE      Use custom config file (default: config.yaml)
    --help, -h         Show this help message

Examples:
    # Initial setup
    $0 init                    # Create config.yaml from template
    nano ../config.yaml        # Edit with your settings
    $0 generate-env            # Generate .env files from config.yaml
    $0 distribute              # Deploy .env files to all machines
    
    # Update .env files after changing config.yaml
    $0 generate-env --force    # Regenerate .env files (does NOT touch config.yaml)
    
    # Troubleshooting  
    $0 check-all               # Run all validation checks
    $0 check-ssh               # Just check SSH connectivity
    $0 check-docker --verbose  # Check Docker with detailed output

    # Advanced
    $0 generate-config --force # Regenerate config.yaml from template (OVERWRITES config.yaml!)
    $0 setup-ssh               # Interactive SSH key setup
    
    # Note: .env.template files are preserved for user reference
EOF
}

# Check if required tools are available
check_requirements() {
    local missing=()
    
    for cmd in ssh scp docker python3 yq; do
        if ! command -v "$cmd" &> /dev/null; then
            missing+=("$cmd")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing required tools: ${missing[*]}"
        echo "" >&2
        echo "Please install missing tools:" >&2
        for cmd in "${missing[@]}"; do
            case "$cmd" in
                yq)
                    echo "  # Install yq (YAML processor) - Ubuntu/Debian" >&2
                    echo "  sudo snap install yq" >&2
                    echo "" >&2
                    echo "  # Alternative: Download binary directly" >&2
                    echo "  # wget -qO /usr/local/bin/yq https://github.com/mikefarah/yq/releases/latest/download/yq_linux_amd64" >&2
                    echo "  # chmod +x /usr/local/bin/yq" >&2
                    ;;
                python3)
                    echo "  # Install Python 3" >&2
                    echo "  sudo apt update && sudo apt install python3 python3-yaml" >&2
                    ;;
                docker)
                    echo "  # Install Docker (Official method)" >&2
                    echo "  curl -fsSL https://get.docker.com | sh" >&2
                    echo "  sudo usermod -aG docker \$USER" >&2
                    echo "" >&2
                    echo "  # Alternative: Ubuntu repository" >&2
                    echo "  # sudo apt update && sudo apt install docker.io" >&2
                    ;;
                *)
                    echo "  # Install $cmd" >&2
                    echo "  sudo apt update && sudo apt install $cmd" >&2
                    ;;
            esac
        done
        echo "" >&2
        return 1
    fi
    
    return 0
}

# Parse YAML config file  
parse_config() {
    local config_file="${1:-$CONFIG_FILE}"
    
    if [[ ! -f "$config_file" ]]; then
        error "Config file not found: $config_file"
        echo ""
        next_step "run ./setup.sh init" "create initial configuration"
        return 1
    fi
    
    # Export config values as environment variables for scripts to use
    eval "$(yq eval '
        "export AGG_HOST=" + .aggregator.host,
        "export AGG_USER=" + .aggregator.user,
        "export AGG_PORT=" + (.aggregator.port | tostring),
        "export AGG_REMOTE_DIR=" + .aggregator.remote_dir,
        "export PERF_DATA_DIR=" + .paths.perf_data_dir,
        "export PROGRAM_CACHE_FILE=" + .paths.program_cache_file,
        "export DOCKER_PREFIX=" + .docker.prefix,
        "export CPUSET_CPUS=" + .numa.cpuset_cpus,
        "export CPUSET_MEMS=" + .numa.cpuset_mems
    ' "$config_file")"
    
    return 0
}

# Extract worker info from config
get_workers() {
    local config_file="${1:-$CONFIG_FILE}"
    yq eval '.workers[] | .host + " " + .user + " " + (.port | tostring) + " " + .worker_id + " " + (.index | tostring) + " " + .remote_dir' "$config_file"
}

# Validate configuration file
validate_config() {
    local config_file="${1:-$CONFIG_FILE}"
    local verbose="${2:-false}"
    
    if [[ "$verbose" == "true" ]]; then
        log "Validating configuration: $config_file"
    fi
    
    if [[ ! -f "$config_file" ]]; then
        error "Configuration file not found: $config_file"
        next_step "run ./setup.sh init" "create initial configuration"
        return 1
    fi
    
    # Check YAML syntax
    if ! yq eval '.' "$config_file" > /dev/null 2>&1; then
        error "Invalid YAML syntax in $config_file"
        return 1  
    fi
    
    # Validate required fields
    local required_fields=(
        ".aggregator.host"
        ".aggregator.user"
        ".aggregator.remote_dir"
        ".aggregator.orchestrator_client_addr"
        ".aggregator.final_aggregator_client_addr"
        ".aggregator.proof_service_addr"
        ".workers"
        ".paths.perf_data_dir"
        ".docker.prefix"
    )
    
    local missing_fields=()
    for field in "${required_fields[@]}"; do
        if ! yq eval "$field" "$config_file" > /dev/null 2>&1; then
            missing_fields+=("$field")
        fi
    done
    
    if [[ ${#missing_fields[@]} -gt 0 ]]; then
        error "Missing required configuration fields:"
        printf '  - %s\n' "${missing_fields[@]}"
        return 1
    fi
    
    # Validate worker configuration
    local worker_count
    worker_count=$(yq eval '.workers | length' "$config_file")
    if [[ "$worker_count" -eq 0 ]]; then
        error "No workers configured"
        return 1
    fi
    
    # Check for duplicate worker indices
    local indices
    indices=$(yq eval '.workers[].index' "$config_file" | sort -n)
    local unique_indices 
    unique_indices=$(echo "$indices" | uniq)
    if [[ "$indices" != "$unique_indices" ]]; then
        error "Duplicate worker indices found"
        return 1
    fi
    
    # Check worker indices are sequential starting from 0
    local expected_indices
    expected_indices=$(seq 0 $((worker_count - 1)) | tr '\n' ' ')
    local actual_indices
    actual_indices=$(echo "$indices" | tr '\n' ' ')
    if [[ "$expected_indices" != "$actual_indices" ]]; then
        error "Worker indices must be sequential starting from 0"
        error "Expected: $expected_indices"
        error "Actual: $actual_indices" 
        return 1
    fi
    
    if [[ "$verbose" == "true" ]]; then
        success "Configuration validation passed"
        info "Aggregator: $(yq eval '.aggregator.host' "$config_file")"
        info "Workers: $worker_count"
    fi
    
    return 0
}

# Check SSH connectivity to a host
check_ssh_host() {
    local host="$1"
    local user="$2"
    local port="${3:-22}"
    local verbose="${4:-false}"
    
    if [[ "$verbose" == "true" ]]; then
        log "Checking SSH connectivity to ${user}@${host}:${port}..."
    fi
    
    local ssh_opts="-o ConnectTimeout=10 -o BatchMode=yes"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    if ssh $ssh_opts "${user}@${host}" "echo 'SSH OK'" > /dev/null 2>&1; then
        if [[ "$verbose" == "true" ]]; then
            success "SSH connectivity OK: ${user}@${host}:${port}"
        fi
        return 0
    else
        error "SSH connectivity failed: ${user}@${host}:${port}"
        return 1
    fi
}

# Check SSH connectivity to all machines
check_ssh_connectivity() {
    local config_file="${1:-$CONFIG_FILE}"
    local verbose="${2:-false}"
    
    log "Checking SSH connectivity to all machines..."
    
    if ! parse_config "$config_file"; then
        return 1
    fi
    
    local failures=0
    
    # Check aggregator
    if ! check_ssh_host "$AGG_HOST" "$AGG_USER" "$AGG_PORT" "$verbose"; then
        ((failures++))
    fi
    
    # Check workers
    while IFS= read -r worker_spec; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        if ! check_ssh_host "$host" "$user" "$port" "$verbose"; then
            ((failures++))
        fi
    done <<< "$(get_workers "$config_file")"
    
    if [[ $failures -eq 0 ]]; then
        success "SSH connectivity check passed"
        return 0
    else
        error "SSH connectivity failed on $failures machine(s)"
        echo ""
        warning "To fix SSH connectivity issues:"
        echo "  1. Set up SSH key authentication: ./setup.sh setup-ssh"
        echo "  2. Check firewall settings and network connectivity"
        echo "  3. Verify hostnames/IPs are correct in config.yaml"
        echo ""
        next_step "run ./setup.sh setup-ssh" "setup SSH key authentication interactively"
        return 1
    fi
}

# Check Docker installation on a host
check_docker_host() {
    local host="$1"
    local user="$2"
    local docker_prefix="$3"
    local port="${4:-22}"
    local verbose="${5:-false}"
    
    if [[ "$verbose" == "true" ]]; then
        log "Checking Docker on ${user}@${host}:${port}..."
    fi
    
    local ssh_opts="-o ConnectTimeout=10"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    # Check if Docker is installed and running
    local docker_status
    if docker_status=$(ssh $ssh_opts "${user}@${host}" "$docker_prefix version --format '{{.Server.Version}}'" 2>/dev/null); then
        if [[ "$verbose" == "true" ]]; then
            success "Docker OK on ${user}@${host}:${port} (version: $docker_status)"
        fi
        return 0
    else
        error "Docker check failed on ${user}@${host}:${port}"
        if [[ "$verbose" == "true" ]]; then
            # Try to get more detailed error info
            ssh $ssh_opts "${user}@${host}" "$docker_prefix version" 2>&1 | head -5 | sed 's/^/  /'
        fi
        return 1
    fi
}

# Check Docker installation on all machines  
check_docker_installation() {
    local config_file="${1:-$CONFIG_FILE}"
    local verbose="${2:-false}"
    
    log "Checking Docker installation on all machines..."
    
    if ! parse_config "$config_file"; then
        return 1
    fi
    
    local docker_prefix
    docker_prefix=$(yq eval '.docker.prefix' "$config_file")
    
    local failures=0
    
    # Check aggregator
    if ! check_docker_host "$AGG_HOST" "$AGG_USER" "$docker_prefix" "$AGG_PORT" "$verbose"; then
        ((failures++))
    fi
    
    # Check workers
    while IFS= read -r worker_spec; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        if ! check_docker_host "$host" "$user" "$docker_prefix" "$port" "$verbose"; then
            ((failures++))
        fi
    done <<< "$(get_workers "$config_file")"
    
    if [[ $failures -eq 0 ]]; then
        success "Docker installation check passed"
        return 0
    else
        error "Docker installation failed on $failures machine(s)"
        echo ""
        warning "To fix Docker issues:"
        echo "  1. Install Docker: curl -fsSL https://get.docker.com | sh"
        echo "  2. Add user to docker group: sudo usermod -aG docker \$USER"
        echo "  3. Start Docker service: sudo systemctl start docker"
        echo "  4. Or use 'sudo docker' by setting docker.prefix: 'sudo docker' in config.yaml"
        return 1
    fi
}

# Check GPU availability on a host
check_gpu_host() {
    local host="$1"
    local user="$2"
    local port="${3:-22}"
    local verbose="${4:-false}"
    
    if [[ "$verbose" == "true" ]]; then
        log "Checking GPU on ${user}@${host}:${port}..."
    fi
    
    local ssh_opts="-o ConnectTimeout=10"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    # Check if nvidia-smi is available and GPUs are detected
    local gpu_info
    if gpu_info=$(ssh $ssh_opts "${user}@${host}" "nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits" 2>/dev/null); then
        local gpu_count
        gpu_count=$(echo "$gpu_info" | wc -l)
        if [[ "$verbose" == "true" ]]; then
            success "GPU OK on ${user}@${host}:${port} ($gpu_count GPU(s))"
            echo "$gpu_info" | sed 's/^/    /'
        fi
        return 0
    else
        error "GPU check failed on ${user}@${host}:${port}"
        return 1
    fi
}

# Check GPU availability on all machines
check_gpu_availability() {
    local config_file="${1:-$CONFIG_FILE}"
    local verbose="${2:-false}"
    
    log "Checking GPU availability on all machines..."
    
    if ! parse_config "$config_file"; then
        return 1
    fi
    
    local failures=0
    
    # Check aggregator
    if ! check_gpu_host "$AGG_HOST" "$AGG_USER" "$AGG_PORT" "$verbose"; then
        ((failures++))
    fi
    
    # Check workers  
    while IFS= read -r worker_spec; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        if ! check_gpu_host "$host" "$user" "$port" "$verbose"; then
            ((failures++))
        fi
    done <<< "$(get_workers "$config_file")"
    
    if [[ $failures -eq 0 ]]; then
        success "GPU availability check passed" 
        return 0
    else
        error "GPU availability failed on $failures machine(s)"
        echo ""
        warning "To fix GPU issues:"
        echo "  1. Install NVIDIA drivers"
        echo "  2. Install nvidia-container-toolkit for Docker GPU support"
        echo "  3. Restart Docker service after installing nvidia-container-toolkit"
        return 1
    fi
}

# Check required paths exist on a host
check_paths_host() {
    local host="$1"
    local user="$2"
    local remote_dir="$3" 
    local perf_data_dir="$4"
    local program_cache="$5"
    local port="${6:-22}"
    local verbose="${7:-false}"
    
    if [[ "$verbose" == "true" ]]; then
        log "Checking paths on ${user}@${host}:${port}..."
    fi
    
    local ssh_opts="-o ConnectTimeout=10"
    if [[ "$port" != "22" ]]; then
        ssh_opts="$ssh_opts -p $port"
    fi
    
    local failures=0
    
    # Check remote directory
    if ! ssh $ssh_opts "${user}@${host}" "test -d '$remote_dir'" 2>/dev/null; then
        error "Remote directory not found on ${user}@${host}:${port}: $remote_dir"
        ((failures++))
    fi
    
    # Check perf data directory  
    if ! ssh $ssh_opts "${user}@${host}" "test -d '$perf_data_dir'" 2>/dev/null; then
        error "Perf data directory not found on ${user}@${host}:${port}: $perf_data_dir"
        ((failures++))
    fi
    
    # Check program cache file (optional)
    if ! ssh $ssh_opts "${user}@${host}" "test -f '$program_cache'" 2>/dev/null; then
        warning "Program cache file not found on ${user}@${host}:${port}: $program_cache (will be created)"
    fi
    
    if [[ $failures -eq 0 ]]; then
        if [[ "$verbose" == "true" ]]; then
            success "Required paths OK on ${user}@${host}:${port}"
        fi
        return 0
    else
        return 1
    fi
}

# Check required paths exist on all machines  
check_required_paths() {
    local config_file="${1:-$CONFIG_FILE}"
    local verbose="${2:-false}"
    
    log "Checking required paths on all machines..."
    
    if ! parse_config "$config_file"; then
        return 1
    fi
    
    local perf_data_dir program_cache
    perf_data_dir=$(yq eval '.paths.perf_data_dir' "$config_file")
    program_cache=$(yq eval '.paths.program_cache_file' "$config_file")
    
    local failures=0
    
    # Check aggregator
    if ! check_paths_host "$AGG_HOST" "$AGG_USER" "$AGG_REMOTE_DIR" "$perf_data_dir" "$program_cache" "$AGG_PORT" "$verbose"; then
        ((failures++))
    fi
    
    # Check workers
    while IFS= read -r worker_spec; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        if ! check_paths_host "$host" "$user" "$remote_dir" "$perf_data_dir" "$program_cache" "$port" "$verbose"; then
            ((failures++))
        fi
    done <<< "$(get_workers "$config_file")"
    
    if [[ $failures -eq 0 ]]; then
        success "Required paths check passed"
        return 0
    else
        error "Required paths failed on $failures machine(s)"
        echo ""
        warning "To fix path issues:"
        echo "  1. Create missing directories on remote machines"
        echo "  2. Update paths in config.yaml to match your setup"
        echo "  3. Ensure perf/bench_data directory contains required files"
        return 1
    fi
}

# Initialize configuration from template
init_config() {
    local force="${1:-false}"
    
    log "Initializing configuration..."
    
    if [[ -f "$CONFIG_FILE" ]] && [[ "$force" != "true" ]]; then
        error "Configuration file already exists: $CONFIG_FILE"
        echo ""
        warning "Use --force to overwrite existing configuration"
        next_step "run ./setup.sh init --force" "overwrite existing configuration"
        return 1
    fi
    
    if [[ ! -f "$CONFIG_TEMPLATE" ]]; then
        error "Configuration template not found: $CONFIG_TEMPLATE"
        return 1
    fi
    
    # Copy template to config file
    cp "$CONFIG_TEMPLATE" "$CONFIG_FILE"
    
    success "Configuration initialized: $CONFIG_FILE"
    echo ""
    info "Next steps:"
    echo "  1. Edit $CONFIG_FILE with your machine IPs and settings"
    echo "  2. Run: ./setup.sh generate-env"
    echo "  3. Run: ./setup.sh validate"
    echo ""
    next_step "edit ../config.yaml and run ./setup.sh generate-env" "configure and generate .env files"
    
    return 0
}

# Generate environment files from config
generate_env_files() {
    local config_file="${1:-$CONFIG_FILE}"
    local dry_run="${2:-false}"
    
    if ! parse_config "$config_file"; then
        return 1
    fi
    
    log "Generating environment files..."
    
    # Generate aggregator .env file
    local agg_env="${PARENT_DIR}/.env.aggregator"
    local workers_csv indices_csv
    
    # Build expected workers and indices lists
    workers_csv=$(yq eval '.workers[].worker_id' "$config_file" | tr '\n' ',' | sed 's/,$//')
    indices_csv=$(yq eval '.workers[].index' "$config_file" | tr '\n' ',' | sed 's/,$//')
    
    if [[ "$dry_run" == "true" ]]; then
        log "Would generate: $agg_env"
    else
        # Check if .env.template files exist and inform user
        if [[ -f "${PARENT_DIR}/.env.aggregator.template" ]]; then
            info "Found .env.aggregator.template - keeping as reference"
        fi
        if [[ -f "${PARENT_DIR}/.env.subblock.template" ]]; then
            info "Found .env.subblock.template - keeping as reference"
        fi
        
        cat > "$agg_env" <<EOF
# =============================================================================
# Aggregator Configuration
# =============================================================================
# Generated by setup.sh from $config_file
# This file contains ONLY network-related settings that users can configure.
# All other settings are pre-configured for experiment reproducibility.
# =============================================================================

# Network Configuration
# ---------------------
# The orchestrator address that aggregator listens on
# Format: HOST:PORT
ORCH_LISTEN_ADDR=$(yq eval '.aggregator.orchestrator_listen_addr' "$config_file")

# The final aggregator service address that accepts deferred proofs
# Format: HOST:PORT
FINAL_AGG_LISTEN_ADDR=$(yq eval '.aggregator.final_aggregator_listen_addr' "$config_file")

# Expected worker IDs (comma-separated list)
# These should match the WORKER_ID values from subblock workers
ORCH_EXPECTED_WORKERS=$workers_csv

# Expected subblock indices (comma-separated list)
ORCH_EXPECTED_INDICES=$indices_csv

# Run ID for tracking this experiment
RUN_ID=$(yq eval '.experiment.run_id' "$config_file")

PROOF_SERVICE_ADDR=$(yq eval '.aggregator.proof_service_addr' "$config_file")

# =============================================================================
# Advanced Settings (Optional - uncomment to override defaults)
# =============================================================================

# GPU Settings (adjust according to your hardware)
# RTX 4090 / RTX 3090:
# SPLIT_THRESHOLD=$(yq eval '.experiment.split_threshold' "$config_file")
# CHUNK_SIZE=$(yq eval '.experiment.retry_chunk_size' "$config_file")

# RTX 5090:
# SPLIT_THRESHOLD=$(yq eval '.experiment.split_threshold' "$config_file")
# CHUNK_SIZE=$(yq eval '.experiment.chunk_size' "$config_file")

# Logging
# RUST_LOG=info
# RUST_LOG=debug

# Resource limits
# PROVER_COUNT=$(yq eval '.experiment.prover_count' "$config_file")
# NUM_THREADS=$(yq eval '.experiment.num_threads' "$config_file")
# CPU_EVENT_POOL_SIZE=$(yq eval '.experiment.cpu_event_pool_size' "$config_file")
EOF
        success "Generated: $agg_env"
    fi
    
    # Generate worker .env file template
    local worker_env="${PARENT_DIR}/.env.subblock"
    
    if [[ "$dry_run" == "true" ]]; then
        log "Would generate: $worker_env (template)"
    else
        # Note: .env.subblock.template check was already done above
        cat > "$worker_env" <<EOF
# =============================================================================
# Subblock Worker Configuration Template
# =============================================================================
# Generated by setup.sh from $config_file
# This template will be customized per worker during distribution.
# This file contains ONLY network-related settings that users can configure.
# All other settings are pre-configured for experiment reproducibility.
# =============================================================================

# Network Configuration
# ---------------------
# The orchestrator address to connect to (should match aggregator's ORCH_LISTEN_ADDR)
# Format: http://HOST:PORT
ORCH_ADDR=$(yq eval '.aggregator.orchestrator_client_addr' "$config_file")

# The final aggregator address to send deferred proofs to
# Format: http://HOST:PORT
FINAL_AGG_ADDR=$(yq eval '.aggregator.final_aggregator_client_addr' "$config_file")

# Worker ID (unique identifier for this worker)
# This should match one of the values in aggregator's ORCH_EXPECTED_WORKERS
WORKER_ID=WORKER_ID_PLACEHOLDER

# Deferred index (which subblock this worker is processing)
# This should be one of the indices in aggregator's ORCH_EXPECTED_INDICES
DEFERRED_INDEX=DEFERRED_INDEX_PLACEHOLDER

# Run ID for tracking this experiment (should match aggregator)
RUN_ID=$(yq eval '.experiment.run_id' "$config_file")

# =============================================================================
# Advanced Settings (Optional - uncomment to override defaults)
# =============================================================================

# GPU Settings (adjust according to your hardware)
# RTX 4090 / RTX 3090:
# SPLIT_THRESHOLD=$(yq eval '.experiment.split_threshold' "$config_file")
# CHUNK_SIZE=$(yq eval '.experiment.retry_chunk_size' "$config_file")

# RTX 5090:
# SPLIT_THRESHOLD=$(yq eval '.experiment.split_threshold' "$config_file")
# CHUNK_SIZE=$(yq eval '.experiment.chunk_size' "$config_file")

# Logging
# RUST_LOG=info
# RUST_LOG=debug

# Resource limits
# PROVER_COUNT=$(yq eval '.experiment.prover_count' "$config_file")
# NUM_THREADS=$(yq eval '.experiment.num_threads' "$config_file")
# CPU_EVENT_POOL_SIZE=$(yq eval '.experiment.cpu_event_pool_size' "$config_file")
EOF
        success "Generated: $worker_env (template)"
    fi
    
    return 0
}

# Distribute .env files to all machines
distribute_env_files() {
    local config_file="${1:-$CONFIG_FILE}"
    local dry_run="${2:-false}"
    local verbose="${3:-false}"
    
    if ! parse_config "$config_file"; then
        return 1
    fi
    
    log "Distributing .env files to all machines..."
    
    # Check that .env files exist locally
    local agg_env="${PARENT_DIR}/.env.aggregator" 
    local worker_env="${PARENT_DIR}/.env.subblock"
    
    # Inform user about template files if they exist
    if [[ -f "${PARENT_DIR}/.env.aggregator.template" ]]; then
        info "Found .env.aggregator.template - using generated .env.aggregator instead"
    fi
    if [[ -f "${PARENT_DIR}/.env.subblock.template" ]]; then
        info "Found .env.subblock.template - using generated .env.subblock instead"
    fi
    
    if [[ ! -f "$agg_env" ]]; then
        error "Aggregator .env file not found: $agg_env"
        next_step "run ./setup.sh generate-env" "generate .env files from configuration"
        return 1
    fi
    
    if [[ ! -f "$worker_env" ]]; then
        error "Worker .env file template not found: $worker_env"
        next_step "run ./setup.sh generate-env" "generate .env files from configuration"
        return 1
    fi
    
    local failures=0
    
    # Distribute aggregator .env file
    if [[ "$dry_run" == "true" ]]; then
        log "Would copy $agg_env to ${AGG_USER}@${AGG_HOST}:${AGG_PORT}:${AGG_REMOTE_DIR}/"
    else
        if [[ "$verbose" == "true" ]]; then
            log "Copying aggregator .env to ${AGG_USER}@${AGG_HOST}:${AGG_PORT}..."
        fi
        
        local scp_opts="-o ConnectTimeout=10"
        if [[ "$AGG_PORT" != "22" ]]; then
            scp_opts="$scp_opts -P $AGG_PORT"
        fi
        
        if scp $scp_opts "$agg_env" "${AGG_USER}@${AGG_HOST}:${AGG_REMOTE_DIR}/" 2>/dev/null; then
            if [[ "$verbose" == "true" ]]; then
                success "Aggregator .env copied to ${AGG_USER}@${AGG_HOST}:${AGG_PORT}"
            fi
        else
            error "Failed to copy aggregator .env to ${AGG_USER}@${AGG_HOST}:${AGG_PORT}"
            ((failures++))
        fi
    fi
    
    # Distribute worker .env files (customized per worker)
    while IFS= read -r worker_spec; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        
        # Create customized worker .env file
        local custom_worker_env="/tmp/.env.subblock.${wid}"
        sed "s/WORKER_ID_PLACEHOLDER/${wid}/g; s/DEFERRED_INDEX_PLACEHOLDER/${idx}/g" "$worker_env" > "$custom_worker_env"
        
        if [[ "$dry_run" == "true" ]]; then
            log "Would copy customized .env.subblock to ${user}@${host}:${port}:${remote_dir}/"
        else
            if [[ "$verbose" == "true" ]]; then
                log "Copying worker .env to ${user}@${host}:${port} (worker: $wid, index: $idx)..."
            fi
            
            local scp_opts="-o ConnectTimeout=10"
            if [[ "$port" != "22" ]]; then
                scp_opts="$scp_opts -P $port"
            fi
            
            if scp $scp_opts "$custom_worker_env" "${user}@${host}:${remote_dir}/.env.subblock" 2>/dev/null; then
                if [[ "$verbose" == "true" ]]; then
                    success "Worker .env copied to ${user}@${host}:${port}"
                fi
            else
                error "Failed to copy worker .env to ${user}@${host}:${port}"
                ((failures++))
            fi
        fi
        
        # Clean up temporary file
        rm -f "$custom_worker_env"
        
    done <<< "$(get_workers "$config_file")"
    
    if [[ $failures -eq 0 ]]; then
        if [[ "$dry_run" != "true" ]]; then
            success "Environment files distributed successfully"
            echo ""
            next_step "run cd scripts && ./docker-multi-control.sh deploy" "deploy Docker images to all machines"
        fi
        return 0
    else
        error "Failed to distribute .env files to $failures machine(s)"
        return 1
    fi
}

# Generate only .env files from existing config.yaml (does NOT touch config.yaml)
generate_env_only() {
    local force="${1:-false}"
    
    if [[ ! -f "$CONFIG_FILE" ]]; then
        error "Config file not found: $CONFIG_FILE"
        echo ""
        next_step "run ./setup.sh init" "create initial configuration"
        return 1
    fi
    
    # Check if .env files already exist
    if [[ -f "${PARENT_DIR}/.env.aggregator" ]] || [[ -f "${PARENT_DIR}/.env.subblock" ]]; then
        if [[ "$force" != "true" ]]; then
            error ".env files already exist. Use --force to overwrite"
            echo ""
            info "Existing files:"
            [[ -f "${PARENT_DIR}/.env.aggregator" ]] && echo "  - .env.aggregator"
            [[ -f "${PARENT_DIR}/.env.subblock" ]] && echo "  - .env.subblock"
            return 1
        fi
    fi
    
    log "Generating .env files from existing config.yaml..."
    
    if generate_env_files "$CONFIG_FILE"; then
        echo ""
        next_step "run ./setup.sh validate" "validate the generated .env files"
        return 0
    else
        return 1
    fi
}

# Generate final config from template and host overrides
# WARNING: This OVERWRITES config.yaml!
generate_config() {
    local force="${1:-false}"
    
    log "WARNING: This will OVERWRITE your config.yaml file!"
    log "Generating configuration from template and host overrides..."
    
    if [[ -f "$CONFIG_FILE" ]] && [[ "$force" != "true" ]]; then
        error "Configuration file already exists: $CONFIG_FILE"
        warning "This command will OVERWRITE your config.yaml!"
        warning "Use --force to overwrite (or use 'generate-env' to only update .env files)"
        echo ""
        next_step "run ./setup.sh generate-env" "generate only .env files without touching config.yaml"
        return 1
    fi
    
    # Confirm overwrite if force is used
    if [[ "$force" == "true" ]] && [[ -f "$CONFIG_FILE" ]]; then
        warning "OVERWRITING existing config.yaml with template..."
    fi
    
    # Start with template
    cp "$CONFIG_TEMPLATE" "$CONFIG_FILE"
    
    # Apply host-specific overrides if they exist
    if [[ -d "$HOSTS_DIR" ]]; then
        log "Applying host-specific overrides from $HOSTS_DIR..."
        
        for override_file in "$HOSTS_DIR"/*.yaml; do
            if [[ -f "$override_file" ]]; then
                local hostname
                hostname=$(basename "$override_file" .yaml)
                log "  Applying overrides for host: $hostname"
                
                # Merge overrides into config (this would need a more sophisticated merge)
                # For now, just log that overrides were found
                info "  Found override file: $override_file (manual merge required)"
            fi
        done
    fi
    
    success "Configuration generated: $CONFIG_FILE"
    
    # Generate .env files
    if generate_env_files "$CONFIG_FILE"; then
        echo ""
        next_step "run ./setup.sh validate" "validate the generated configuration"
    fi
    
    return 0
}


# Interactive SSH setup
setup_ssh_keys() {
    log "Setting up SSH key authentication..."
    echo ""
    
    # Check if SSH key exists
    if [[ ! -f ~/.ssh/id_rsa ]] && [[ ! -f ~/.ssh/id_ed25519 ]]; then
        info "No SSH key found. Generating new ED25519 key..."
        ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -N ""
        success "SSH key generated: ~/.ssh/id_ed25519"
    else
        success "SSH key already exists"
    fi
    
    # Determine which key to use
    local pubkey_file
    if [[ -f ~/.ssh/id_ed25519.pub ]]; then
        pubkey_file="~/.ssh/id_ed25519.pub"
    elif [[ -f ~/.ssh/id_rsa.pub ]]; then
        pubkey_file="~/.ssh/id_rsa.pub"
    else
        error "No SSH public key found"
        return 1
    fi
    
    log "Public key file: $pubkey_file"
    echo ""
    
    if [[ ! -f "$CONFIG_FILE" ]]; then
        error "Configuration file not found: $CONFIG_FILE"
        next_step "run ./setup.sh init" "create initial configuration"
        return 1
    fi
    
    # Parse config to get hosts
    if ! parse_config "$CONFIG_FILE"; then
        return 1
    fi
    
    log "Copying SSH key to all machines..."
    echo ""
    
    # Copy to aggregator
    info "Setting up SSH for aggregator: ${AGG_USER}@${AGG_HOST}:${AGG_PORT}"
    local ssh_copy_opts="-o ConnectTimeout=30"
    if [[ "$AGG_PORT" != "22" ]]; then
        ssh_copy_opts="$ssh_copy_opts -p $AGG_PORT"
    fi
    if ssh-copy-id $ssh_copy_opts "${AGG_USER}@${AGG_HOST}"; then
        success "SSH key copied to ${AGG_USER}@${AGG_HOST}:${AGG_PORT}"
    else
        warning "Failed to copy SSH key to ${AGG_USER}@${AGG_HOST}:${AGG_PORT}"
    fi
    
    # Copy to workers
    while IFS= read -r worker_spec; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        info "Setting up SSH for worker $wid: ${user}@${host}:${port}"
        local worker_ssh_copy_opts="-o ConnectTimeout=30"
        if [[ "$port" != "22" ]]; then
            worker_ssh_copy_opts="$worker_ssh_copy_opts -p $port"
        fi
        if ssh-copy-id $worker_ssh_copy_opts "${user}@${host}"; then
            success "SSH key copied to ${user}@${host}:${port}"
        else
            warning "Failed to copy SSH key to ${user}@${host}:${port}"
        fi
    done <<< "$(get_workers "$CONFIG_FILE")"
    
    echo ""
    success "SSH setup complete!"
    echo ""
    next_step "run ./setup.sh check-ssh" "verify SSH connectivity"
    
    return 0
}

# Clean up generated files
clean_generated_files() {
    local force="${1:-false}"
    
    local files_to_clean=(
        "$CONFIG_FILE"
        "${PARENT_DIR}/.env.aggregator"
        "${PARENT_DIR}/.env.subblock"
    )
    
    # Note: .env.template files are preserved for user reference
    
    if [[ "$force" != "true" ]]; then
        echo "This will remove the following files:"
        for file in "${files_to_clean[@]}"; do
            if [[ -f "$file" ]]; then
                echo "  - $file"
            fi
        done
        echo ""
        read -p "Are you sure? [y/N] " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log "Cancelled"
            return 0
        fi
    fi
    
    local removed=0
    for file in "${files_to_clean[@]}"; do
        if [[ -f "$file" ]]; then
            rm "$file"
            success "Removed: $file"
            ((removed++))
        fi
    done
    
    if [[ $removed -eq 0 ]]; then
        info "No generated files to clean"
    else
        success "Cleaned $removed file(s)"
        echo ""
        next_step "run ./setup.sh init" "reinitialize configuration"
    fi
    
    return 0
}

# Main command dispatcher
main() {
    local command="${1:-}"
    local verbose="false"
    local dry_run="false"  
    local force="false"
    local config_file="$CONFIG_FILE"
    
    # Parse global options
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --verbose|-v)
                verbose="true"
                shift
                ;;
            --dry-run|-n)
                dry_run="true"
                shift
                ;;
            --force|-f)
                force="true"
                shift
                ;;
            --config)
                config_file="$2"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            --*)
                error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                if [[ -z "$command" ]]; then
                    command="$1"
                fi
                shift
                ;;
        esac
    done
    
    if [[ -z "$command" ]]; then
        usage
        exit 1
    fi
    
    # Ensure required tools are available (except for init and clean)
    if [[ "$command" != "init" ]] && [[ "$command" != "clean" ]]; then
        if ! check_requirements; then
            error "Cannot proceed without required tools. Please install them and try again."
            exit 1
        fi
    fi
    
    case "$command" in
        init)
            init_config "$force"
            ;;
        validate)  
            log "Validating configuration..."
            if validate_config "$config_file" "$verbose"; then
                success "Configuration validation passed"
                echo ""
                info "Config file: $config_file"
                info "Aggregator: $(yq eval '.aggregator.host' "$config_file" 2>/dev/null || echo 'not found')"
                info "Workers: $(yq eval '.workers | length' "$config_file" 2>/dev/null || echo '0')"
                echo ""
                next_step "run ./setup.sh generate-env" "generate .env files from configuration"
                exit 0
            else
                error "Configuration validation failed"
                exit 1
            fi
            ;;
        check-ssh)
            check_ssh_connectivity "$config_file" "$verbose"
            ;;
        check-docker)
            check_docker_installation "$config_file" "$verbose"  
            ;;
        check-gpu)
            check_gpu_availability "$config_file" "$verbose"
            ;;
        check-paths)
            check_required_paths "$config_file" "$verbose"
            ;;
        check-all)
            local all_passed="true"
            validate_config "$config_file" "$verbose" || all_passed="false"
            check_ssh_connectivity "$config_file" "$verbose" || all_passed="false" 
            check_docker_installation "$config_file" "$verbose" || all_passed="false"
            check_gpu_availability "$config_file" "$verbose" || all_passed="false"
            check_required_paths "$config_file" "$verbose" || all_passed="false"
            
            if [[ "$all_passed" == "true" ]]; then
                success "All checks passed"
                next_step "run cd scripts && ./docker-multi-control.sh deploy" "deploy Docker images"
            else
                error "Some checks failed"
                exit 1
            fi
            ;;
        generate-env)
            generate_env_only "$force"
            ;;
        generate-config)
            generate_config "$force"
            ;;
        distribute) 
            distribute_env_files "$config_file" "$dry_run" "$verbose"
            ;;
        setup-ssh)
            setup_ssh_keys
            ;;
        clean)
            clean_generated_files "$force"
            ;;
        *)
            error "Unknown command: $command"
            usage
            exit 1
            ;;
    esac
}

main "$@"
