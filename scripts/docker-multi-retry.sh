#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Retry Script
# =============================================================================
# Retry a failed block with smaller CHUNK_SIZE
# This script:
# 1. Saves aggregator logs from the failed run
# 2. Stops all containers
# 3. Updates .env files with smaller CHUNK_SIZE (2^21 instead of 2^22)
# 4. Restarts all containers with the new configuration
# 
# Note: The code automatically resets CHUNK_SIZE back to normal after processing
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Retry a failed block with smaller CHUNK_SIZE.

This script will:
1. Save logs from the current run
2. Force kill all containers with verification (retries until all removed)
3. Verify all containers are completely gone before proceeding
4. Update .env files with CHUNK_SIZE=${CHUNK_SIZE_RETRY} (2^21)
5. Restart all containers

Note: This ONLY modifies CHUNK_SIZE in the .env files. All other parameters
(BLOCK_NUMBER, GAS_THRESHOLD, etc.) remain unchanged.

The code automatically resets CHUNK_SIZE back to ${CHUNK_SIZE_NORMAL} (2^22) after
processing the problematic block.

Exit Codes:
    0    Success - All containers restarted cleanly (Rust program can proceed)
    1    Failure - Cleanup or restart failed (manual intervention required)

Options:
    --chunk-size NUM        Custom retry chunk size (default: ${CHUNK_SIZE_RETRY})
    --wait-time SEC         Wait time between cleanup and start (default: 3)
    --cleanup-retries NUM   Max cleanup retry attempts (default: 3)
    --help, -h              Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)

Examples:
    # Retry with default smaller chunk size
    $0

    # Retry with custom chunk size and more retries
    $0 --chunk-size 1048576 --cleanup-retries 5

    # For use in Rust program (capture exit code):
    if ./docker-multi-retry.sh; then
        echo "Retry successful, proceed with next steps"
    else
        echo "Retry failed, manual intervention required"
    fi

    # To reset CHUNK_SIZE to normal, use:
    docker-multi-control.sh reset-chunk-size
EOF
}

# Update environment file with retry configuration
update_env_chunk_size() {
    local host="$1"
    local user="$2"
    local env_file="$3"
    local chunk_size="$4"
    local port="${5:-22}"
    
    log "Updating $env_file on ${user}@${host} with CHUNK_SIZE=$chunk_size..."
    
    # Add or update CHUNK_SIZE if it doesn't exist
    ssh_exec "$user" "$host" "$port" "
        cd \$(dirname '$env_file')
        if grep -q '^CHUNK_SIZE=' '$env_file' 2>/dev/null; then
            sed -i 's/^CHUNK_SIZE=.*/CHUNK_SIZE=$chunk_size/' '$env_file'
        elif grep -q '^# CHUNK_SIZE=' '$env_file' 2>/dev/null; then
            sed -i 's/^# CHUNK_SIZE=.*/CHUNK_SIZE=$chunk_size/' '$env_file'
        else
            echo 'CHUNK_SIZE=$chunk_size' >> '$env_file'
        fi
    "
}

update_all_env_files() {
    local chunk_size="$1"
    
    log "Updating all .env files with CHUNK_SIZE=$chunk_size"
    
    # Update aggregator env
    local agg_env="${AGG_REMOTE_DIR}/${ENV_FILE_AGGREGATOR}"
    update_env_chunk_size "$AGG_HOST" "$AGG_USER" "$agg_env" "$chunk_size" "$AGG_PORT"
    
    # Update worker envs
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        local worker_env="${remote_dir}/${ENV_FILE_WORKER}"
        update_env_chunk_size "$host" "$user" "$worker_env" "$chunk_size" "$port"
    done
    
    log "All .env files updated"
}

save_all_logs() {
    local timestamp=$(date +"$TIMESTAMP_FORMAT")
    local log_prefix="${1:-retry}"
    
    log "Saving logs from failed run..."
    
    # Save aggregator logs
    local agg_log="${AGG_REMOTE_DIR}/${LOGS_DIR}/aggregator-${log_prefix}-${timestamp}.log"
    save_container_logs "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$agg_log" "$AGG_PORT" || true
    
    # Optionally save worker logs
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        local worker_log="${remote_dir}/${LOGS_DIR}/subblock-${wid}-${log_prefix}-${timestamp}.log"
        save_container_logs "$host" "$user" "$CONTAINER_NAME_WORKER" "$worker_log" "$port" || true
    done
}

main() {
    local chunk_size="$CHUNK_SIZE_RETRY"
    local wait_time=3
    local cleanup_max_retries=3
    local cleanup_retry_delay=2
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --chunk-size)
                chunk_size="$2"
                shift 2
                ;;
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
    
    log "=== Docker Multi-Machine Retry (Force Kill Mode) ==="
    log "Retry CHUNK_SIZE: $chunk_size (normal: $CHUNK_SIZE_NORMAL)"
    log "Cleanup max retries: $cleanup_max_retries"
    log "Note: Only CHUNK_SIZE will be modified in .env files"
    echo ""
    
    # Step 1: Save logs (with error handling for SSH failures)
    log "Step 1/5: Saving logs from failed run..."
    save_all_logs "failed" || warn "Some logs could not be saved (containers may already be stopped)"
    echo ""
    
    # Step 2: Force kill containers with retry and verification
    log "Step 2/5: Force killing and removing all containers..."
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
    
    # Step 3: Update .env files
    log "Step 3/5: Updating .env files with CHUNK_SIZE=$chunk_size..."
    if ! update_all_env_files "$chunk_size"; then
        error "Failed to update .env files"
        exit 1
    fi
    echo ""
    
    # Step 4: Wait for cleanup
    log "Step 4/5: Waiting ${wait_time}s for system cleanup..."
    sleep "$wait_time"
    echo ""
    
    # Step 5: Start all containers
    log "Step 5/5: Starting all containers with retry configuration..."
    if ! start_all; then
        error "Failed to start containers"
        exit 1
    fi
    echo ""
    
    log "=== Retry Configuration Applied ==="
    log "Containers restarted with CHUNK_SIZE=$chunk_size"
    log "After processing completes, CHUNK_SIZE will auto-reset to $CHUNK_SIZE_NORMAL"
    echo ""
    
    show_all_status
    echo ""
    
    log "=== Retry Complete - SUCCESS ==="
    log "Exit code: 0 (ready for Rust program to proceed)"
    exit 0
}

main "$@"
