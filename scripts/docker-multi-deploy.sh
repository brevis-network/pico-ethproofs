#!/usr/bin/env bash
# =============================================================================
# Docker Multi-Machine Deploy Script
# =============================================================================
# Deploy Docker images to all machines (aggregator + workers)
# Handles image distribution, cleanup, and loading
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/docker-common.sh"

# Default image file paths (relative to script directory or absolute)
DEFAULT_AGG_IMAGE="${SCRIPT_DIR}/../pico-aggregator.tar.gz"
DEFAULT_WORKER_IMAGE="${SCRIPT_DIR}/../pico-subblock-worker.tar.gz"

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Deploy Docker images to all machines (aggregator and workers).

This script will:
1. Verify image files exist locally
2. Copy images to remote machines
3. Clean up old containers and images
4. Load new images
5. Verify deployment

Options:
    --agg-image PATH        Path to aggregator image (default: ../pico-aggregator.tar.gz)
    --worker-image PATH     Path to worker image (default: ../pico-subblock-worker.tar.gz)
    --agg-only              Deploy only to aggregator
    --workers-only          Deploy only to workers
    --skip-cleanup          Don't remove old containers/images
    --keep-tar              Keep tar files on remote machines after loading
    --help, -h              Show this help message

Environment Variables:
    AGG_HOST            Aggregator host IP (default: 10.23.101.63)
    AGG_USER            Aggregator SSH user (default: ubuntu)
    DOCKER_PREFIX       Docker command prefix (default: sudo docker)

Image File Formats:
    - Compressed: .tar.gz (recommended)
    - Uncompressed: .tar

Examples:
    # Deploy both images with default paths
    $0

    # Deploy with custom image paths
    $0 --agg-image /path/to/aggregator.tar.gz --worker-image /path/to/worker.tar.gz

    # Deploy only aggregator
    $0 --agg-only --agg-image pico-aggregator.tar.gz

    # Deploy without cleanup (keep old images)
    $0 --skip-cleanup
EOF
}

# Check if a file is compressed
is_compressed() {
    local file="$1"
    [[ "$file" == *.gz ]] || [[ "$file" == *.tgz ]]
}

# Get the load command based on file type
get_load_command() {
    local file="$1"
    if is_compressed "$file"; then
        echo "gunzip -c '$file' | $DOCKER_PREFIX load"
    else
        echo "$DOCKER_PREFIX load -i '$file'"
    fi
}

# Verify image file exists locally
verify_local_image() {
    local image_path="$1"
    local image_type="$2"
    
    if [[ ! -f "$image_path" ]]; then
        error "$image_type image not found: $image_path"
        return 1
    fi
    
    local size=$(du -h "$image_path" | cut -f1)
    log "Found $image_type image: $image_path (size: $size)"
    return 0
}

# Clean up old containers and images on a remote machine
cleanup_remote_docker() {
    local host="$1"
    local user="$2"
    local container_name="$3"
    local image_name="$4"
    local port="${5:-22}"
    
    log "Cleaning up old containers and images on ${user}@${host}..."
    
    # Stop and remove container if exists
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX stop $container_name 2>/dev/null || true"
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rm $container_name 2>/dev/null || true"
    
    # Remove old image if exists
    ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX rmi $image_name 2>/dev/null || true"
    
    log "Cleanup complete on ${user}@${host}"
}

# Deploy image to a remote machine
deploy_image_to_machine() {
    local host="$1"
    local user="$2"
    local local_image="$3"
    local remote_dir="$4"
    local container_name="$5"
    local image_name="$6"
    local machine_type="$7"  # "aggregator" or "worker"
    local port="${8:-22}"
    
    log "=== Deploying $machine_type image to ${user}@${host} ==="
    
    # Get remote image filename
    local image_filename=$(basename "$local_image")
    local remote_image="${remote_dir}/${image_filename}"
    
    # Step 1: Copy image to remote machine
    log "Step 1/3: Copying image to ${user}@${host}..."
    if ! scp_copy "$local_image" "$user" "$host" "$remote_image" "$port"; then
        error "Failed to copy image to ${user}@${host}"
        return 1
    fi
    log "Image copied successfully"
    
    # Step 2: Clean up old containers and images
    log "Step 2/3: Cleaning up old containers and images..."
    cleanup_remote_docker "$host" "$user" "$container_name" "$image_name" "$port"
    
    # Step 3: Load new image
    log "Step 3/3: Loading new image..."
    local load_cmd=$(get_load_command "$remote_image")
    if ! ssh_exec "$user" "$host" "$port" "cd '$remote_dir' && $load_cmd"; then
        error "Failed to load image on ${user}@${host}"
        return 1
    fi
    log "Image loaded successfully"
    
    # Verify image is loaded
    if ! ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX images | grep -q '${image_name%:*}'"; then
        error "Image verification failed on ${user}@${host}"
        return 1
    fi
    log "Image verified: $image_name"
    
    log "=== Deployment to ${user}@${host} complete ==="
    return 0
}

# Deploy to aggregator
deploy_aggregator() {
    local agg_image="$1"
    local skip_cleanup="$2"
    local keep_tar="$3"
    
    log "=== Deploying Aggregator ==="
    
    # Verify local image
    if ! verify_local_image "$agg_image" "Aggregator"; then
        return 1
    fi
    
    # Deploy image
    if ! deploy_image_to_machine \
        "$AGG_HOST" \
        "$AGG_USER" \
        "$agg_image" \
        "$AGG_REMOTE_DIR" \
        "$CONTAINER_NAME_AGGREGATOR" \
        "$IMAGE_NAME_AGGREGATOR" \
        "aggregator" \
        "$AGG_PORT"; then
        error "Aggregator deployment failed"
        return 1
    fi
    
    # Remove tar file if requested
    if [[ "$keep_tar" != "true" ]]; then
        local image_filename=$(basename "$agg_image")
        log "Removing tar file from aggregator..."
        ssh_exec "$AGG_USER" "$AGG_HOST" "rm -f '${AGG_REMOTE_DIR}/${image_filename}'" || true
    fi
    
    log "=== Aggregator Deployment Complete ==="
    return 0
}

# Deploy to all workers
deploy_workers() {
    local worker_image="$1"
    local skip_cleanup="$2"
    local keep_tar="$3"
    
    log "=== Deploying Workers ==="
    
    # Verify local image
    if ! verify_local_image "$worker_image" "Worker"; then
        return 1
    fi
    
    local failed_workers=()
    
    # Deploy to each worker
    for worker_spec in "${WORKERS[@]}"; do
        read -r host user port wid idx remote_dir <<< "$worker_spec"
        
        log ""
        log "Deploying to worker $wid..."
        
        if ! deploy_image_to_machine \
            "$host" \
            "$user" \
            "$worker_image" \
            "$remote_dir" \
            "$CONTAINER_NAME_WORKER" \
            "$IMAGE_NAME_WORKER" \
            "worker $wid" \
            "$port"; then
            error "Worker $wid deployment failed"
            failed_workers+=("$wid")
        else
            # Remove tar file if requested
            if [[ "$keep_tar" != "true" ]]; then
                local image_filename=$(basename "$worker_image")
                log "Removing tar file from worker $wid..."
                ssh_exec "$user" "$host" "$port" "rm -f '${remote_dir}/${image_filename}'" || true
            fi
        fi
        
        apply_worker_delay
    done
    
    # Check for failures
    if [[ ${#failed_workers[@]} -gt 0 ]]; then
        error "Failed to deploy to ${#failed_workers[@]} worker(s): ${failed_workers[*]}"
        return 1
    fi
    
    log ""
    log "=== All Workers Deployed Successfully ==="
    return 0
}

# Verify deployment on all machines
verify_deployment() {
    local mode="$1"
    
    log "=== Verifying Deployment ==="
    
    local failures=0
    
    # Verify aggregator
    if [[ "$mode" == "all" ]] || [[ "$mode" == "agg" ]]; then
        log "Verifying aggregator image..."
        if ssh_exec "$AGG_USER" "$AGG_HOST" "$AGG_PORT" "$DOCKER_PREFIX images | grep -q '${IMAGE_NAME_AGGREGATOR%:*}'"; then
            log "✓ Aggregator image verified: $IMAGE_NAME_AGGREGATOR"
        else
            error "✗ Aggregator image not found!"
            ((failures++))
        fi
    fi
    
    # Verify workers
    if [[ "$mode" == "all" ]] || [[ "$mode" == "workers" ]]; then
        for worker_spec in "${WORKERS[@]}"; do
            read -r host user port wid idx remote_dir <<< "$worker_spec"
            log "Verifying worker $wid image..."
            if ssh_exec "$user" "$host" "$port" "$DOCKER_PREFIX images | grep -q '${IMAGE_NAME_WORKER%:*}'"; then
                log "✓ Worker $wid image verified: $IMAGE_NAME_WORKER"
            else
                error "✗ Worker $wid image not found!"
                ((failures++))
            fi
        done
    fi
    
    if [[ $failures -eq 0 ]]; then
        log "=== Deployment Verification Passed ==="
        return 0
    else
        error "=== Deployment Verification Failed ($failures failures) ==="
        return 1
    fi
}

main() {
    local agg_image="$DEFAULT_AGG_IMAGE"
    local worker_image="$DEFAULT_WORKER_IMAGE"
    local mode="all"
    local skip_cleanup="false"
    local keep_tar="false"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --agg-image)
                agg_image="$2"
                shift 2
                ;;
            --worker-image)
                worker_image="$2"
                shift 2
                ;;
            --agg-only)
                mode="agg"
                shift
                ;;
            --workers-only)
                mode="workers"
                shift
                ;;
            --skip-cleanup)
                skip_cleanup="true"
                shift
                ;;
            --keep-tar)
                keep_tar="true"
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
    
    # Validate configuration
    if ! validate_config; then
        error "Configuration validation failed"
        exit 1
    fi
    
    # Initialize SSH connection multiplexing
    init_ssh_control
    
    log "=== Docker Multi-Machine Deploy ==="
    log "Mode: $mode"
    log "Skip cleanup: $skip_cleanup"
    log "Keep tar files: $keep_tar"
    log "Aggregator: ${AGG_USER}@${AGG_HOST}"
    log "Workers: ${#WORKERS[@]}"
    echo ""
    
    # Deploy based on mode
    local deploy_success=true
    
    case "$mode" in
        all)
            if ! deploy_aggregator "$agg_image" "$skip_cleanup" "$keep_tar"; then
                deploy_success=false
            fi
            echo ""
            if ! deploy_workers "$worker_image" "$skip_cleanup" "$keep_tar"; then
                deploy_success=false
            fi
            ;;
        agg)
            if ! deploy_aggregator "$agg_image" "$skip_cleanup" "$keep_tar"; then
                deploy_success=false
            fi
            ;;
        workers)
            if ! deploy_workers "$worker_image" "$skip_cleanup" "$keep_tar"; then
                deploy_success=false
            fi
            ;;
    esac
    
    echo ""
    
    # Verify deployment
    if [[ "$deploy_success" == "true" ]]; then
        if verify_deployment "$mode"; then
            log ""
            log "=== Deployment Complete - SUCCESS ==="
            log "Images are ready. Use 'docker-multi-control.sh start' to start containers."
            exit 0
        else
            error "=== Deployment Complete - VERIFICATION FAILED ==="
            exit 1
        fi
    else
        error "=== Deployment Failed ==="
        exit 1
    fi
}

main "$@"

