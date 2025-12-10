#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Fast Retry Script
# =============================================================================
# Fast retry for proving failures - no log saving, no CHUNK_SIZE changes
# This script:
# 1. Force kills all containers with verification
# 2. Waits for cleanup
# 3. Restarts all containers with existing configuration
# 
# Unlike docker-multi-retry.sh, this script:
# - Does NOT save logs (faster)
# - Does NOT modify CHUNK_SIZE (keeps current value)
# - Minimizes downtime for quick recovery
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Fast retry for proving failures without log saving or CHUNK_SIZE changes.

This script will:
1. Force kill all containers with verification (retries until all removed)
2. Wait for cleanup
3. Restart all containers with existing configuration

This is optimized for speed and does NOT:
- Save logs from the failed run
- Modify CHUNK_SIZE in .env files
- Change any other configuration parameters

Exit Codes:
    0    Success - All containers restarted cleanly (Rust program can proceed)
    1    Failure - Cleanup or restart failed (manual intervention required)

Options:
    --wait-time SEC         Wait time between cleanup and start (default: 3)
    --cleanup-retries NUM   Max cleanup retry attempts (default: 3)
    --help, -h              Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)

Examples:
    # Fast retry with defaults
    $0

    # Fast retry with custom wait time
    $0 --wait-time 5

    # For use in Rust program (capture exit code):
    if ./docker-multi-fast-retry.sh; then
        echo "Fast retry successful, proceed with next steps"
    else
        echo "Fast retry failed, manual intervention required"
    fi
EOF
}

main() {
    local wait_time=3
    local cleanup_max_retries=3
    local cleanup_retry_delay=2
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --wait-time)
                wait_time="$2"
                shift 2
                ;;
            --cleanup-retries)
                cleanup_max_retries="$2"
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
    
    log "=== Docker Multi-Machine Fast Retry (Force Kill Mode) ==="
    log "Cleanup max retries: $cleanup_max_retries"
    log "Note: NO log saving, NO CHUNK_SIZE changes"
    echo ""
    
    # Step 1: Force kill containers with retry and verification
    log "Step 1/3: Force killing and removing all containers..."
    local cleanup_retry=0
    local cleanup_success=false
    
    while [[ $cleanup_retry -lt $cleanup_max_retries ]]; do
        log "Cleanup attempt $((cleanup_retry + 1))/$cleanup_max_retries"
        
        # Force kill all containers
        if force_kill_all; then
            log "Force kill completed, verifying removal..."
            
            # Verify all containers are gone
            if verify_all_containers_gone; then
                cleanup_success=true
                log "All containers successfully removed and verified"
                break
            else
                warn "Some containers still exist after force kill"
            fi
        else
            warn "Force kill reported failures"
        fi
        
        cleanup_retry=$((cleanup_retry + 1))
        if [[ $cleanup_retry -lt $cleanup_max_retries ]]; then
            warn "Retrying cleanup in ${cleanup_retry_delay}s..."
            sleep "$cleanup_retry_delay"
        fi
    done
    
    if [[ "$cleanup_success" != "true" ]]; then
        error "Failed to completely remove all containers after $cleanup_max_retries attempts"
        error "Cannot proceed with restart - manual intervention required"
        exit 1
    fi
    echo ""
    
    # Step 2: Wait for cleanup
    log "Step 2/3: Waiting ${wait_time}s for system cleanup..."
    sleep "$wait_time"
    echo ""
    
    # Step 3: Start all containers
    log "Step 3/3: Starting all containers with existing configuration..."
    if ! start_all; then
        error "Failed to start containers"
        exit 1
    fi
    echo ""
    
    log "=== Fast Retry Complete ==="
    log "Containers restarted with existing configuration"
    echo ""
    
    show_all_status
    echo ""
    
    log "=== Fast Retry Complete - SUCCESS ==="
    log "Exit code: 0 (ready for Rust program to proceed)"
    exit 0
}

main "$@"

