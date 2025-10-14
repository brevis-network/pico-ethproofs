#!/usr/bin/env bash
# =============================================================================
# Build and Compress Docker Images
# =============================================================================
# Personal script to build Docker images and compress them for deployment
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"

# Configuration
EXPIRY_DURATION="${EXPIRY_DURATION:-32d}"
OUTPUT_DIR="${OUTPUT_DIR:-$DOCKER_DIR}"

log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    echo "[ERROR] $*" >&2
    exit 1
}

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Build Docker images and compress them for deployment.

Options:
    --expiry DURATION       Certificate expiry duration (default: 32d)
    --output-dir DIR        Output directory for compressed images (default: $DOCKER_DIR)
    --aggregator-only       Build only aggregator image
    --worker-only           Build only worker image
    --help, -h              Show this help message

Environment Variables:
    EXPIRY_DURATION         Certificate expiry duration (default: 32d)
    OUTPUT_DIR              Output directory for compressed images

Examples:
    # Build and compress all images
    $0

    # Build with custom expiry
    $0 --expiry 60d

    # Build only aggregator
    $0 --aggregator-only

    # Custom output directory
    $0 --output-dir ~/images
EOF
}

build_aggregator() {
    local expiry="$1"
    
    log "=== Building Aggregator Image ==="
    log "Expiry duration: $expiry"
    
    cd "$DOCKER_DIR"
    if ! EXPIRY_DURATION="$expiry" make build-aggregator; then
        error "Failed to build aggregator image"
    fi
    
    log "Aggregator image built successfully"
}

build_worker() {
    local expiry="$1"
    
    log "=== Building Worker Image ==="
    log "Expiry duration: $expiry"
    
    cd "$DOCKER_DIR"
    if ! EXPIRY_DURATION="$expiry" make build-subblock; then
        error "Failed to build worker image"
    fi
    
    log "Worker image built successfully"
}

compress_image() {
    local image_name="$1"
    local output_file="$2"
    
    log "Compressing $image_name to $output_file..."
    
    if ! docker save "$image_name" | gzip > "$output_file"; then
        error "Failed to compress $image_name"
    fi
    
    local size=$(du -h "$output_file" | cut -f1)
    log "Compressed image saved: $output_file (size: $size)"
}

main() {
    local expiry="$EXPIRY_DURATION"
    local output_dir="$OUTPUT_DIR"
    local mode="all"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --expiry)
                expiry="$2"
                shift 2
                ;;
            --output-dir)
                output_dir="$2"
                shift 2
                ;;
            --aggregator-only)
                mode="aggregator"
                shift
                ;;
            --worker-only)
                mode="worker"
                shift
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done
    
    log "=== Build and Compress Docker Images ==="
    log "Mode: $mode"
    log "Expiry duration: $expiry"
    log "Output directory: $output_dir"
    echo ""
    
    # Create output directory if needed
    mkdir -p "$output_dir"
    
    # Build and compress based on mode
    case "$mode" in
        all)
            # Build aggregator
            build_aggregator "$expiry"
            echo ""
            
            # Build worker
            build_worker "$expiry"
            echo ""
            
            # Compress aggregator
            log "=== Compressing Images ==="
            compress_image "pico-aggregator:latest" "$output_dir/pico-aggregator.tar.gz"
            echo ""
            
            # Compress worker
            compress_image "pico-subblock-worker:latest" "$output_dir/pico-subblock-worker.tar.gz"
            ;;
            
        aggregator)
            build_aggregator "$expiry"
            echo ""
            log "=== Compressing Image ==="
            compress_image "pico-aggregator:latest" "$output_dir/pico-aggregator.tar.gz"
            ;;
            
        worker)
            build_worker "$expiry"
            echo ""
            log "=== Compressing Image ==="
            compress_image "pico-subblock-worker:latest" "$output_dir/pico-subblock-worker.tar.gz"
            ;;
    esac
    
    echo ""
    log "=== Build and Compress Complete ==="
    log ""
    log "Compressed images:"
    
    if [[ "$mode" == "all" ]] || [[ "$mode" == "aggregator" ]]; then
        ls -lh "$output_dir/pico-aggregator.tar.gz" 2>/dev/null | awk '{print "  - " $9 " (" $5 ")"}'
    fi
    
    if [[ "$mode" == "all" ]] || [[ "$mode" == "worker" ]]; then
        ls -lh "$output_dir/pico-subblock-worker.tar.gz" 2>/dev/null | awk '{print "  - " $9 " (" $5 ")"}'
    fi
    
    echo ""
    log "Next steps:"
    log "  cd scripts"
    log "  ./docker-multi-control.sh deploy"
}

main "$@"

