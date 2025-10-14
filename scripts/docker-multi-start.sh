#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Start Script
# =============================================================================
# Start all Docker containers (aggregator + subblock workers) remotely
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Start all Docker containers (aggregator and subblock workers) on remote machines.

Options:
    --agg-only          Start only the aggregator
    --workers-only      Start only the workers
    --cleanup           Clean up existing containers before starting
    --help, -h          Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)
    PERF_DATA_DIR       Path to perf/bench_data directory
    CPUSET_CPUS         CPU set for NUMA binding (default: 62-123)
    CPUSET_MEMS         Memory node for NUMA binding (default: 1)

Examples:
    # Start all containers
    $0

    # Clean up and start (useful if containers already exist)
    $0 --cleanup

    # Start only aggregator
    $0 --agg-only

    # Start only workers
    $0 --workers-only

    # Use custom Docker prefix
    DOCKER_PREFIX="docker" $0
EOF
}

main() {
    local mode="all"
    local do_cleanup="false"
    
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
            --cleanup)
                do_cleanup="true"
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
    
    log "=== Docker Multi-Machine Start ==="
    log "Mode: $mode"
    log "Cleanup first: $do_cleanup"
    log "Aggregator: ${AGG_USER}@${AGG_HOST}"
    log "Workers: ${#WORKERS[@]}"
    log "Docker prefix: $DOCKER_PREFIX"
    log "NUMA settings: cpus=$CPUSET_CPUS mems=$CPUSET_MEMS"
    
    # Clean up existing containers if requested
    if [[ "$do_cleanup" == "true" ]]; then
        log "Cleaning up existing containers..."
        case "$mode" in
            all)
                cleanup_all
                ;;
            agg)
                cleanup_aggregator
                ;;
            workers)
                cleanup_all_workers
                ;;
        esac
        sleep 2
    fi
    
    case "$mode" in
        all)
            start_all
            ;;
        agg)
            start_aggregator
            ;;
        workers)
            start_all_workers
            ;;
    esac
    
    echo ""
    show_all_status
    
    log "=== Start Complete ==="
}

main "$@"

