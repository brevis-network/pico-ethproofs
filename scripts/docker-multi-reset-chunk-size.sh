#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Reset CHUNK_SIZE Script
# =============================================================================
# Reset CHUNK_SIZE back to normal value in all .env files
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Reset CHUNK_SIZE back to normal value (${CHUNK_SIZE_NORMAL}) in all .env files.

This is useful when:
- A retry didn't complete successfully
- You want to manually reset the environment
- You want to ensure default chunk size before starting

Options:
    --chunk-size NUM    Custom chunk size to set (default: ${CHUNK_SIZE_NORMAL})
    --restart           Restart containers after resetting chunk size
    --help, -h          Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)

Examples:
    # Reset to default chunk size
    $0

    # Reset and restart containers
    $0 --restart

    # Set custom chunk size
    $0 --chunk-size 4194304
EOF
}

# Update environment file with chunk size
reset_env_chunk_size() {
    local host="$1"
    local user="$2"
    local env_file="$3"
    local chunk_size="$4"
    local port="${5:-22}"
    
    log "Resetting $env_file on ${user}@${host} to CHUNK_SIZE=$chunk_size..."
    
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

reset_all_env_files() {
    local chunk_size="$1"
    
    log "Resetting all .env files to CHUNK_SIZE=$chunk_size"
    
    # Update aggregator env
    local agg_env="${AGG_REMOTE_DIR}/${ENV_FILE_AGGREGATOR}"
    reset_env_chunk_size "$AGG_HOST" "$AGG_USER" "$agg_env" "$chunk_size" "$AGG_PORT"
    
    # Update worker envs
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        local worker_env="${remote_dir}/${ENV_FILE_WORKER}"
        reset_env_chunk_size "$host" "$user" "$worker_env" "$chunk_size" "$port"
    done
    
    log "All .env files reset"
}

main() {
    local chunk_size="$CHUNK_SIZE_NORMAL"
    local do_restart="false"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --chunk-size)
                chunk_size="$2"
                shift 2
                ;;
            --restart)
                do_restart="true"
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
    
    log "=== Docker Multi-Machine Reset CHUNK_SIZE ==="
    log "Target CHUNK_SIZE: $chunk_size (normal: $CHUNK_SIZE_NORMAL)"
    log "Restart after reset: $do_restart"
    
    # Reset .env files
    reset_all_env_files "$chunk_size"
    
    # Restart if requested
    if [[ "$do_restart" == "true" ]]; then
        log ""
        log "Restarting all containers with reset CHUNK_SIZE..."
        restart_all
    else
        log ""
        log "CHUNK_SIZE reset complete. Restart containers to apply changes:"
        log "  docker-multi-control.sh restart"
    fi
    
    log "=== Reset Complete ==="
}

main "$@"

