#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Multi-Machine Deployment Script
# =============================================================================
# This script automates deployment of aggregator and subblock workers across
# multiple machines via SSH.
# =============================================================================

# Configuration
AGG_HOST="10.23.101.63"  # Aggregator machine IP
AGG_USER="ubuntu"        # SSH user for aggregator

# Subblock worker machines (format: "HOST:USER:WORKER_ID:INDEX")
WORKERS=(
    "10.23.101.64:ubuntu:worker1:0"
    "10.23.101.65:ubuntu:worker2:1"
    "10.23.101.66:ubuntu:worker3:2"
    "10.23.101.67:ubuntu:worker4:3"
    "10.23.101.68:ubuntu:worker5:4"
    "10.23.101.69:ubuntu:worker6:5"
    "10.23.101.70:ubuntu:worker7:6"
)

# Experiment configuration
BLOCK_NUMBER="23290000"
GAS_THRESHOLD="10000000"
RUN_ID="block-${BLOCK_NUMBER}"

# Data directory on remote machines
# This should be the path to perf/bench_data on each machine
REMOTE_DATA_DIR="/data/brevis/perf/bench_data"

# Docker image files (relative to this script)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"
AGG_IMAGE_FILE="${DOCKER_DIR}/pico-aggregator-latest.tar.gz"
WORKER_IMAGE_FILE="${DOCKER_DIR}/pico-subblock-worker-latest.tar.gz"

# Remote directory
REMOTE_DIR="/tmp/brevis-multi-machine"

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    echo "[ERROR] $*" >&2
    exit 1
}

check_prerequisites() {
    log "Checking prerequisites..."
    
    if [[ ! -f "$AGG_IMAGE_FILE" ]]; then
        error "Aggregator image not found: $AGG_IMAGE_FILE. Run 'make save-images' first."
    fi
    
    if [[ ! -f "$WORKER_IMAGE_FILE" ]]; then
        error "Worker image not found: $WORKER_IMAGE_FILE. Run 'make save-images' first."
    fi
    
    log "Prerequisites OK"
}

deploy_aggregator() {
    log "Deploying aggregator to ${AGG_USER}@${AGG_HOST}..."
    
    # Create remote directory
    ssh "${AGG_USER}@${AGG_HOST}" "mkdir -p ${REMOTE_DIR}"
    
    # Copy image
    log "Copying aggregator image..."
    scp "$AGG_IMAGE_FILE" "${AGG_USER}@${AGG_HOST}:${REMOTE_DIR}/"
    
    # Load image
    log "Loading aggregator image..."
    ssh "${AGG_USER}@${AGG_HOST}" "docker load -i ${REMOTE_DIR}/pico-aggregator-latest.tar.gz"
    
    # Build expected workers and indices lists
    local worker_ids=()
    local indices=()
    for worker_spec in "${WORKERS[@]}"; do
        IFS=':' read -r host user wid idx <<< "$worker_spec"
        worker_ids+=("$wid")
        indices+=("$idx")
    done
    
    local workers_csv=$(IFS=,; echo "${worker_ids[*]}")
    local indices_csv=$(IFS=,; echo "${indices[*]}")
    
    # Create .env file
    log "Creating aggregator configuration..."
    ssh "${AGG_USER}@${AGG_HOST}" "cat > ${REMOTE_DIR}/.env.aggregator" <<EOF
ORCH_LISTEN_ADDR=0.0.0.0:50052
FINAL_AGG_LISTEN_ADDR=0.0.0.0:50051
ORCH_EXPECTED_WORKERS=${workers_csv}
ORCH_EXPECTED_INDICES=${indices_csv}
BLOCK_NUMBER=${BLOCK_NUMBER}
GAS_THRESHOLD=${GAS_THRESHOLD}
RUN_ID=${RUN_ID}
EOF
    
    # Stop existing container if running
    ssh "${AGG_USER}@${AGG_HOST}" "docker stop pico-aggregator 2>/dev/null || true"
    ssh "${AGG_USER}@${AGG_HOST}" "docker rm pico-aggregator 2>/dev/null || true"
    
    # Start aggregator
    log "Starting aggregator..."
    ssh "${AGG_USER}@${AGG_HOST}" "docker run -d \
        --name pico-aggregator \
        --gpus all \
        --network host \
        --env-file ${REMOTE_DIR}/.env.aggregator \
        -v ${REMOTE_DATA_DIR}:/app/perf/bench_data:ro \
        pico-aggregator:latest"
    
    log "Aggregator deployed successfully"
}

deploy_worker() {
    local host="$1"
    local user="$2"
    local worker_id="$3"
    local index="$4"
    
    log "Deploying worker ${worker_id} (index ${index}) to ${user}@${host}..."
    
    # Create remote directory
    ssh "${user}@${host}" "mkdir -p ${REMOTE_DIR}"
    
    # Copy image
    log "Copying worker image to ${host}..."
    scp "$WORKER_IMAGE_FILE" "${user}@${host}:${REMOTE_DIR}/"
    
    # Load image
    log "Loading worker image on ${host}..."
    ssh "${user}@${host}" "docker load -i ${REMOTE_DIR}/pico-subblock-worker-latest.tar.gz"
    
    # Create .env file
    log "Creating worker ${worker_id} configuration..."
    ssh "${user}@${host}" "cat > ${REMOTE_DIR}/.env.subblock" <<EOF
ORCH_ADDR=http://${AGG_HOST}:50052
FINAL_AGG_ADDR=http://${AGG_HOST}:50051
WORKER_ID=${worker_id}
DEFERRED_INDEX=${index}
BLOCK_NUMBER=${BLOCK_NUMBER}
GAS_THRESHOLD=${GAS_THRESHOLD}
RUN_ID=${RUN_ID}
EOF
    
    # Stop existing container if running
    ssh "${user}@${host}" "docker stop pico-subblock-worker 2>/dev/null || true"
    ssh "${user}@${host}" "docker rm pico-subblock-worker 2>/dev/null || true"
    
    # Start worker
    log "Starting worker ${worker_id}..."
    ssh "${user}@${host}" "docker run -d \
        --name pico-subblock-worker \
        --gpus all \
        --network host \
        --env-file ${REMOTE_DIR}/.env.subblock \
        -v ${REMOTE_DATA_DIR}:/app/perf/bench_data:ro \
        pico-subblock-worker:latest"
    
    log "Worker ${worker_id} deployed successfully"
}

deploy_all_workers() {
    log "Deploying ${#WORKERS[@]} workers..."
    
    for worker_spec in "${WORKERS[@]}"; do
        IFS=':' read -r host user wid idx <<< "$worker_spec"
        deploy_worker "$host" "$user" "$wid" "$idx"
    done
    
    log "All workers deployed successfully"
}

show_status() {
    log "Checking deployment status..."
    
    log "Aggregator status:"
    ssh "${AGG_USER}@${AGG_HOST}" "docker ps | grep pico-aggregator || echo 'Not running'"
    
    for worker_spec in "${WORKERS[@]}"; do
        IFS=':' read -r host user wid idx <<< "$worker_spec"
        log "Worker ${wid} status:"
        ssh "${user}@${host}" "docker ps | grep pico-subblock-worker || echo 'Not running'"
    done
}

stop_all() {
    log "Stopping all containers..."
    
    log "Stopping aggregator..."
    ssh "${AGG_USER}@${AGG_HOST}" "docker stop pico-aggregator 2>/dev/null || true"
    
    for worker_spec in "${WORKERS[@]}"; do
        IFS=':' read -r host user wid idx <<< "$worker_spec"
        log "Stopping worker ${wid}..."
        ssh "${user}@${host}" "docker stop pico-subblock-worker 2>/dev/null || true"
    done
    
    log "All containers stopped"
}

show_logs() {
    local target="${1:-aggregator}"
    
    if [[ "$target" == "aggregator" ]]; then
        log "Showing aggregator logs (Ctrl-C to exit)..."
        ssh "${AGG_USER}@${AGG_HOST}" "docker logs -f pico-aggregator"
    elif [[ "$target" =~ ^worker([0-9]+)$ ]]; then
        local worker_num="${BASH_REMATCH[1]}"
        local idx=$((worker_num - 1))
        
        if [[ $idx -lt ${#WORKERS[@]} ]]; then
            IFS=':' read -r host user wid _ <<< "${WORKERS[$idx]}"
            log "Showing ${wid} logs (Ctrl-C to exit)..."
            ssh "${user}@${host}" "docker logs -f pico-subblock-worker"
        else
            error "Invalid worker number: $worker_num"
        fi
    else
        error "Invalid target: $target. Use 'aggregator' or 'worker1', 'worker2', etc."
    fi
}

usage() {
    cat <<EOF
Usage: $0 [COMMAND]

Commands:
    deploy-all          Deploy aggregator and all workers
    deploy-aggregator   Deploy only aggregator
    deploy-workers      Deploy only workers
    status              Show deployment status
    logs [TARGET]       Show logs (aggregator, worker1, worker2, etc.)
    stop                Stop all containers
    help                Show this help message

Environment Variables:
    BLOCK_NUMBER       Block number to process (default: 23290000)
    GAS_THRESHOLD      Gas threshold (default: 10000000)
    
Examples:
    $0 deploy-all
    $0 status
    $0 logs aggregator
    $0 logs worker1
    $0 stop
    
Edit this script to configure machine IPs and worker assignments.
EOF
}

main() {
    local command="${1:-help}"
    
    case "$command" in
        deploy-all)
            check_prerequisites
            deploy_aggregator
            sleep 5  # Give aggregator time to start
            deploy_all_workers
            show_status
            ;;
        deploy-aggregator)
            check_prerequisites
            deploy_aggregator
            show_status
            ;;
        deploy-workers)
            check_prerequisites
            deploy_all_workers
            show_status
            ;;
        status)
            show_status
            ;;
        logs)
            show_logs "${2:-aggregator}"
            ;;
        stop)
            stop_all
            ;;
        help|--help|-h)
            usage
            ;;
        *)
            error "Unknown command: $command. Use '$0 help' for usage."
            ;;
    esac
}

main "$@"

