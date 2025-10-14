#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Stop Script
# =============================================================================
# Stop all Docker containers (aggregator + subblock workers) remotely
# Handles zombie processes with retry logic
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Stop all Docker containers (aggregator and subblock workers) on remote machines.
Includes retry logic to handle zombie processes.

Options:
    --agg-only          Stop only the aggregator
    --workers-only      Stop only the workers
    --no-logs           Don't save logs before stopping
    --help, -h          Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)
    STOP_MAX_RETRIES    Maximum retries for zombie processes (default: 5)
    STOP_RETRY_DELAY    Delay between retries in seconds (default: 3)
    LOGS_DIR            Directory for saving logs (default: docker-logs)

Examples:
    # Stop all containers and save logs
    $0

    # Stop only aggregator without saving logs
    $0 --agg-only --no-logs

    # Stop with custom retry settings
    STOP_MAX_RETRIES=10 STOP_RETRY_DELAY=5 $0
EOF
}

main() {
    local mode="all"
    local save_logs="true"
    
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
    
    log "=== Docker Multi-Machine Stop ==="
    log "Mode: $mode"
    log "Save logs: $save_logs"
    log "Stop max retries: $STOP_MAX_RETRIES"
    log "Stop retry delay: ${STOP_RETRY_DELAY}s"
    
    case "$mode" in
        all)
            stop_all "$save_logs"
            ;;
        agg)
            stop_aggregator "$save_logs"
            ;;
        workers)
            stop_all_workers "$save_logs"
            ;;
    esac
    
    echo ""
    show_all_status
    
    log "=== Stop Complete ==="
}

main "$@"

