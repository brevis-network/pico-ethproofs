#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Restart Script
# =============================================================================
# Restart all Docker containers (aggregator + subblock workers) remotely
# Saves logs before stopping, then restarts
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Restart all Docker containers (aggregator and subblock workers) on remote machines.
This will:
1. Save logs from running containers
2. Stop all containers (with retry logic for zombies)
3. Wait briefly for cleanup
4. Start all containers again

Options:
    --agg-only          Restart only the aggregator
    --workers-only      Restart only the workers
    --no-logs           Don't save logs before stopping
    --wait-time SEC     Wait time between stop and start (default: 3)
    --help, -h          Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)
    LOGS_DIR            Directory for saving logs (default: docker-logs)

Examples:
    # Restart all containers
    $0

    # Restart only aggregator with 10s wait
    $0 --agg-only --wait-time 10

    # Quick restart without saving logs
    $0 --no-logs --wait-time 1
EOF
}

main() {
    local mode="all"
    local save_logs="true"
    local wait_time="${RESTART_WAIT_TIME}"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --agg-only)
                mode="agg"
                shift
                ;;
            --workers-only)
                mode="workers"
                shift
                ;;
            --no-logs)
                save_logs="false"
                shift
                ;;
            --wait-time)
                wait_time="$2"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
    
    # Initialize SSH connection multiplexing
    init_ssh_control
    
    log "=== Docker Multi-Machine Restart ==="
    log "Mode: $mode"
    log "Save logs: $save_logs"
    log "Wait time: ${wait_time}s"
    
    case "$mode" in
        all)
            log "Stopping all containers..."
            stop_all "$save_logs"
            log "Waiting ${wait_time}s for cleanup..."
            sleep "$wait_time"
            log "Starting all containers..."
            start_all
            ;;
        agg)
            log "Restarting aggregator..."
            stop_aggregator "$save_logs"
            log "Waiting ${wait_time}s for cleanup..."
            sleep "$wait_time"
            start_aggregator
            ;;
        workers)
            log "Restarting workers..."
            stop_all_workers "$save_logs"
            log "Waiting ${wait_time}s for cleanup..."
            sleep "$wait_time"
            start_all_workers
            ;;
    esac
    
    echo ""
    show_all_status
    
    log "=== Restart Complete ==="
}

main "$@"

