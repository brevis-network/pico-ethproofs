#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Control Script
# =============================================================================
# Unified control interface for multi-machine Docker deployment
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

usage() {
    cat <<EOF
Usage: $0 COMMAND [OPTIONS]

Unified control interface for Docker multi-machine deployment.

Commands:
    deploy              Deploy Docker images to all machines
    remove-images       Remove aggregator and worker images on all machines
    start               Start all containers
    stop                Stop all containers (graceful)
    force-kill          Force kill all containers (immediate)
    restart             Restart all containers
    retry               Retry with smaller CHUNK_SIZE (force kill mode)
    reset-chunk-size    Reset CHUNK_SIZE to normal value
    status              Show container status
    logs                Show container logs
    save-logs           Save logs without stopping
    cleanup             Force remove all containers (running or stopped)

Options (vary by command):
    --agg-only          Target only aggregator
    --workers-only      Target only workers
    --no-logs           Don't save logs
    --chunk-size NUM    Custom chunk size (for retry/reset)
    --restart           Restart after reset (for reset-chunk-size)
    --save              Save logs to file instead of showing (for logs command)
    --help, -h          Show this help message

Examples:
    # Deploy images to all machines
    $0 deploy

    # Remove images from all machines (containers will be removed if needed)
    $0 remove-images

    # Start all containers
    $0 start

    # Clean up existing containers first
    $0 cleanup

    # Stop all and save logs
    $0 stop

    # Restart only aggregator
    $0 restart --agg-only

    # Retry failed block
    $0 retry --chunk-size 2097152

    # Reset CHUNK_SIZE to normal
    $0 reset-chunk-size

    # Reset CHUNK_SIZE and restart
    $0 reset-chunk-size --restart

    # Check status
    $0 status

    # Show aggregator logs (live)
    $0 logs aggregator

    # Save aggregator logs to file (for long logs)
    $0 logs --save aggregator

For detailed help on each command:
    $0 COMMAND --help
EOF
}

cmd_deploy() {
    "${SCRIPT_DIR}/docker-multi-deploy.sh" "$@"
}

cmd_remove_images() {
    init_ssh_control
    local mode="all"

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
            --help|-h)
                cat <<EOF
Usage: docker-multi-control.sh remove-images [OPTIONS]

Remove aggregator and worker Docker images on remote machines. If a container
depends on an image, the container will be stopped and removed before retrying
image removal.

Options:
    --agg-only          Remove images only on aggregator
    --workers-only      Remove images only on workers
    --help, -h          Show this help message

Examples:
    # Remove images on all machines
    docker-multi-control.sh remove-images

    # Remove images only on workers
    docker-multi-control.sh remove-images --workers-only
EOF
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    log "=== Docker Multi-Machine Remove Images ==="
    log "Mode: $mode"

    # Helper to remove image with dependency handling
    remove_image_with_dependencies() {
        local host="$1"
        local user="$2"
        local port="$3"
        local container_name="$4"
        local image_name="$5"

        log "Attempting to remove image $image_name on ${user}@${host}..."
        if ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rmi $image_name >/dev/null 2>&1"; then
            log "Image $image_name removed on ${user}@${host}"
            return 0
        fi

        warn "Image $image_name in use on ${user}@${host}, removing dependent container $container_name..."
        stop_and_remove_container "$host" "$user" "$container_name" "$port" || true

        if ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rmi $image_name >/dev/null 2>&1"; then
            log "Image $image_name removed on ${user}@${host}"
            return 0
        else
            warn "Failed to remove $image_name on ${user}@${host} (it may not exist)"
            return 1
        fi
    }

    case "$mode" in
        all|agg)
            log "Removing aggregator image on ${AGG_USER}@${AGG_HOST}..."
            remove_image_with_dependencies "$AGG_HOST" "$AGG_USER" "$AGG_PORT" "$CONTAINER_NAME_AGGREGATOR" "$IMAGE_NAME_AGGREGATOR" || true
            ;;
    esac

    case "$mode" in
        all|workers)
            for worker_spec in "${WORKERS[@]}"; do
                read -r host user port wid idx remote_dir <<< "$worker_spec"
                log "Removing worker image on ${user}@${host} (worker $wid)..."
                remove_image_with_dependencies "$host" "$user" "$port" "$CONTAINER_NAME_WORKER" "$IMAGE_NAME_WORKER" || true
                apply_worker_delay
            done
            ;;
    esac

    log "=== Remove Images Complete ==="
}

cmd_start() {
    "${SCRIPT_DIR}/docker-multi-start.sh" "$@"
}

cmd_stop() {
    "${SCRIPT_DIR}/docker-multi-stop.sh" "$@"
}

cmd_restart() {
    "${SCRIPT_DIR}/docker-multi-restart.sh" "$@"
}

cmd_retry() {
    "${SCRIPT_DIR}/docker-multi-retry.sh" "$@"
}

cmd_reset_chunk_size() {
    "${SCRIPT_DIR}/docker-multi-reset-chunk-size.sh" "$@"
}

cmd_status() {
    init_ssh_control
    log "=== Docker Multi-Machine Status ==="
    show_all_status
}

cmd_logs() {
    init_ssh_control
    local target="${1:-aggregator}"
    local save_to_file="${2:-false}"
    
    case "$target" in
        aggregator|agg)
            if [[ "$save_to_file" == "true" ]]; then
                local timestamp=$(date +"$TIMESTAMP_FORMAT")
                local log_file="${AGG_REMOTE_DIR}/${LOGS_DIR}/aggregator-live-${timestamp}.log"
                log "Saving aggregator logs to $log_file..."
                ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "$DOCKER_PREFIX logs $CONTAINER_NAME_AGGREGATOR &> '$log_file'"
                log "Logs saved to $log_file"
            else
                log "Following aggregator logs (Ctrl-C to exit)..."
                ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "$DOCKER_PREFIX logs -f $CONTAINER_NAME_AGGREGATOR"
            fi
            ;;
        worker*)
            # Extract worker number (e.g., worker1 -> 0)
            if [[ "$target" =~ ^worker([0-9]+)$ ]]; then
                local worker_num="${BASH_REMATCH[1]}"
                local idx=$((worker_num - 1))
                
                if [[ $idx -lt ${#WORKERS[@]} ]]; then
                    read -r host user port wid deferred_idx remote_dir <<< "${WORKERS[$idx]}"
                    if [[ "$save_to_file" == "true" ]]; then
                        local timestamp=$(date +"$TIMESTAMP_FORMAT")
                        local log_file="${remote_dir}/${LOGS_DIR}/subblock-${wid}-live-${timestamp}.log"
                        log "Saving ${wid} logs to $log_file..."
                        ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX logs $CONTAINER_NAME_WORKER &> '$log_file'"
                        log "Logs saved to $log_file"
                    else
                        log "Following ${wid} logs (Ctrl-C to exit)..."
                        ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX logs -f $CONTAINER_NAME_WORKER"
                    fi
                else
                    error "Invalid worker number: $worker_num (max: ${#WORKERS[@]})"
                    exit 1
                fi
            else
                error "Invalid worker format: $target (use worker1, worker2, etc.)"
                exit 1
            fi
            ;;
        *)
            error "Unknown target: $target (use 'aggregator' or 'worker1', 'worker2', etc.)"
            exit 1
            ;;
    esac
}

cmd_save_logs() {
    init_ssh_control
    local timestamp=$(date +"$TIMESTAMP_FORMAT")
    
    log "=== Saving All Logs ==="
    
    # Save aggregator logs
    local agg_log="${AGG_REMOTE_DIR}/${LOGS_DIR}/aggregator-manual-${timestamp}.log"
    save_container_logs "$AGG_HOST" "$AGG_USER" "$CONTAINER_NAME_AGGREGATOR" "$agg_log" "$AGG_PORT" || true
    
    # Save worker logs
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        local worker_log="${remote_dir}/${LOGS_DIR}/subblock-${wid}-manual-${timestamp}.log"
        save_container_logs "$host" "$user" "$CONTAINER_NAME_WORKER" "$worker_log" "$port" || true
    done
    
    log "=== Logs Saved ==="
}

cmd_cleanup() {
    init_ssh_control
    local mode="all"
    
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
            --help|-h)
                cat <<EOF
Usage: docker-multi-control.sh cleanup [OPTIONS]

Force remove all containers (running or stopped) without saving logs.
Useful for cleaning up before starting new containers.

Options:
    --agg-only      Clean up only aggregator
    --workers-only  Clean up only workers
    --help, -h      Show this help message

Examples:
    # Clean up all containers
    docker-multi-control.sh cleanup

    # Clean up only aggregator
    docker-multi-control.sh cleanup --agg-only
EOF
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    log "=== Docker Multi-Machine Cleanup ==="
    log "Mode: $mode"
    
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
    
    log "=== Cleanup Complete ==="
}

cmd_force_kill() {
    init_ssh_control
    local mode="all"
    
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
            --help|-h)
                cat <<EOF
Usage: docker-multi-control.sh force-kill [OPTIONS]

Force kill all containers immediately without saving logs.
This is faster than graceful stop and useful when containers are stuck.

Options:
    --agg-only      Force kill only aggregator
    --workers-only  Force kill only workers
    --help, -h      Show this help message

Examples:
    # Force kill all containers
    docker-multi-control.sh force-kill

    # Force kill only workers
    docker-multi-control.sh force-kill --workers-only
EOF
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    log "=== Docker Multi-Machine Force Kill ==="
    log "Mode: $mode"
    
    case "$mode" in
        all)
            force_kill_all
            ;;
        agg)
            force_kill_aggregator
            ;;
        workers)
            force_kill_all_workers
            ;;
    esac
    
    log "=== Force Kill Complete ==="
}

main() {
    if [[ $# -eq 0 ]]; then
        usage
        exit 0
    fi
    
    local command="$1"
    shift
    
    case "$command" in
        deploy)
            cmd_deploy "$@"
            ;;
        remove-images)
            cmd_remove_images "$@"
            ;;
        start)
            cmd_start "$@"
            ;;
        stop)
            cmd_stop "$@"
            ;;
        force-kill)
            cmd_force_kill "$@"
            ;;
        restart)
            cmd_restart "$@"
            ;;
        retry)
            cmd_retry "$@"
            ;;
        reset-chunk-size)
            cmd_reset_chunk_size "$@"
            ;;
        status)
            cmd_status "$@"
            ;;
        logs)
            # Parse logs command arguments
            local save_to_file="false"
            local target="aggregator"
            
            # Parse arguments
            while [[ $# -gt 0 ]]; do
                case "$1" in
                    --save)
                        save_to_file="true"
                        shift
                        ;;
                    --help|-h)
                        cat <<EOF
Usage: docker-multi-control.sh logs [OPTIONS] [TARGET]

Show container logs.

Options:
    --save              Save logs to file instead of showing
    --help, -h          Show this help message

Targets:
    aggregator          Show aggregator logs
    worker1, worker2, etc.  Show worker logs

Examples:
    # Show aggregator logs (live)
    docker-multi-control.sh logs aggregator

    # Save aggregator logs to file
    docker-multi-control.sh logs --save aggregator

    # Show worker1 logs (live)
    docker-multi-control.sh logs worker1

    # Save worker1 logs to file
    docker-multi-control.sh logs --save worker1
EOF
                        exit 0
                        ;;
                    *)
                        target="$1"
                        shift
                        ;;
                esac
            done
            
            cmd_logs "$target" "$save_to_file"
            ;;
        save-logs)
            cmd_save_logs "$@"
            ;;
        cleanup)
            cmd_cleanup "$@"
            ;;
        help|--help|-h)
            usage
            exit 0
            ;;
        *)
            error "Unknown command: $command"
            echo ""
            usage
            exit 1
            ;;
    esac
}

main "$@"

