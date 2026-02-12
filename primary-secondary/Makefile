.PHONY: up-primary up-secondary down save-images load-images \
        logs-primary logs-secondary status help

# ============================================================================
# Configuration
# ============================================================================

IMAGE_NAME       ?= pico-two-machine
TAG              ?= latest

# NUMA configuration (default: NUMA node 1 for target servers)
CPUSET_CPUS      ?= 120-239
CPUSET_MEMS      ?= 1

# Block input directory (read-only, contains .bin files for each block number)
# Only bins/ is mounted — reth-elf and vk_digest.bin are baked into the image
BLOCK_INPUT_DIR  ?= ./bins

# Program cache file (read-write, speeds up proving across restarts)
PROGRAM_CACHE_FILE ?= ./program_cache.bin

# Docker command (override with DOCKER=sudo docker if needed)
DOCKER           ?= docker

# ============================================================================
# Run
# ============================================================================

up-primary: ## Start primary container (requires .env.primary)
	@test -f .env.primary || (echo "Error: .env.primary not found. Copy from .env.primary.template" && exit 1)
	$(DOCKER) run -d \
		--name pico-primary \
		--gpus all \
		--network host \
		--ipc=host \
		--cap-add=SYS_NICE \
		--ulimit memlock=-1:-1 \
		--env-file .env.primary \
		--cpuset-cpus='$(CPUSET_CPUS)' \
		--cpuset-mems='$(CPUSET_MEMS)' \
		-v $(BLOCK_INPUT_DIR):/app/perf/bench_data/bins:ro \
		-v $(PROGRAM_CACHE_FILE):/app/program_cache.bin:rw \
		$(IMAGE_NAME):$(TAG)

up-secondary: ## Start secondary container (requires .env.secondary)
	@test -f .env.secondary || (echo "Error: .env.secondary not found. Copy from .env.secondary.template" && exit 1)
	$(DOCKER) run -d \
		--name pico-secondary \
		--gpus all \
		--network host \
		--ipc=host \
		--cap-add=SYS_NICE \
		--ulimit memlock=-1:-1 \
		--env-file .env.secondary \
		--cpuset-cpus='$(CPUSET_CPUS)' \
		--cpuset-mems='$(CPUSET_MEMS)' \
		-v $(BLOCK_INPUT_DIR):/app/perf/bench_data/bins:ro \
		-v $(PROGRAM_CACHE_FILE):/app/program_cache.bin:rw \
		$(IMAGE_NAME):$(TAG)

# ============================================================================
# Stop
# ============================================================================

down: ## Stop and remove all containers
	-$(DOCKER) rm -f pico-primary 2>/dev/null
	-$(DOCKER) rm -f pico-secondary 2>/dev/null

# ============================================================================
# Image Management
# ============================================================================

save-images: ## Export runtime image to .tar.gz
	$(DOCKER) save $(IMAGE_NAME):$(TAG) | gzip > $(IMAGE_NAME)-$(TAG).tar.gz
	@echo "Saved: $(IMAGE_NAME)-$(TAG).tar.gz ($$(du -h $(IMAGE_NAME)-$(TAG).tar.gz | cut -f1))"

load-images: ## Load runtime image from .tar.gz
	@test -f $(IMAGE_NAME)-$(TAG).tar.gz || (echo "Error: $(IMAGE_NAME)-$(TAG).tar.gz not found" && exit 1)
	gunzip -c $(IMAGE_NAME)-$(TAG).tar.gz | $(DOCKER) load
	@echo "Loaded: $(IMAGE_NAME):$(TAG)"

# ============================================================================
# Monitoring
# ============================================================================

logs-primary: ## Follow primary container logs
	$(DOCKER) logs -f pico-primary

logs-secondary: ## Follow secondary container logs
	$(DOCKER) logs -f pico-secondary

status: ## Show container status
	@echo "=== Containers ==="
	@$(DOCKER) ps -a --filter "name=pico-primary" --filter "name=pico-secondary" \
		--format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
	@echo ""
	@echo "=== Images ==="
	@$(DOCKER) images --filter "reference=$(IMAGE_NAME)" \
		--format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"

# ============================================================================
# Help
# ============================================================================

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
