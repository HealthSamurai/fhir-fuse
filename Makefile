.PHONY: help build run release clean test check fmt lint install-targets build-all clean-cache list-cache cache-stats docker-build-arm64 docker-push-arm64

# Default target
help:
	@echo "Available targets:"
	@echo "  make build          - Build the project in debug mode"
	@echo "  make run            - Run the project in debug mode"
	@echo "  make release        - Build the project in release mode"
	@echo "  make run-release    - Run the project in release mode"
	@echo "  make run-native     - Run the native macOS release binary"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make test           - Run tests"
	@echo "  make test-linux-musl-arm64 - Run tests for Linux ARM64 musl (Alpine)"
	@echo "  make check          - Check code without building"
	@echo "  make fmt            - Format code with rustfmt"
	@echo "  make lint           - Run clippy linter"
	@echo "  make check-docker    - Check if Docker is running"
	@echo "  make build-all       - Build for all platforms (macOS, Linux, Windows)"
	@echo ""
	@echo "Platform-specific builds:"
	@echo "  make build-macos-native    - Build for current macOS platform"
	@echo "  make build-linux-x64       - Build for Linux x86_64 (glibc)"
	@echo "  make build-linux-arm64     - Build for Linux ARM64 (glibc)"
	@echo "  make build-linux-musl-x64  - Build for Linux x86_64 (musl, Alpine compatible)"
	@echo "  make build-linux-musl-arm64 - Build for Linux ARM64 (musl, Alpine compatible)"
	@echo "  make build-alpine-x64      - Alias for build-linux-musl-x64"
	@echo "  make build-alpine-arm64    - Alias for build-linux-musl-arm64"
	@echo "  make build-windows-x64     - Windows (not supported - FUSE unavailable)"
	@echo ""
	@echo "Docker image management:"
	@echo "  make docker-build-arm64    - Build Docker image for ARM64 Linux"
	@echo "  make docker-push-arm64     - Push Docker image for ARM64 Linux to registry"
	@echo ""
	@echo "Cache management:"
	@echo "  make list-cache            - List all Docker volume caches"
	@echo "  make cache-stats           - Show cache sizes"
	@echo "  make clean-cache           - Remove all Docker volume caches"

# Basic commands
build:
	cargo build

run:
	cargo run

release:
	cargo build --release

run-release:
	cargo run --release

run-native:
	@./target/aarch64-apple-darwin/release/fhir-fuse

clean:
	cargo clean

test:
	cargo test

test-linux-musl-arm64:
	@echo "Testing for Linux ARM64 musl (Alpine Linux compatible) (using Docker)..."
	docker run --rm --platform linux/arm64 \
		-v "$(PWD)":/workspace \
		-v cargo-cache-musl-arm64:/usr/local/cargo/registry \
		-v cargo-git-cache-musl-arm64:/usr/local/cargo/git \
		-w /workspace \
		rust:alpine \
		sh -c "apk add --no-cache fuse-dev fuse-static pkgconfig && cargo test"
	@echo "✅ Tests completed for Linux ARM64 musl"

check:
	cargo check

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings

# Cross-compilation setup
check-docker:
	@echo "Checking if Docker is running..."
	@docker ps > /dev/null 2>&1 || (echo "Error: Docker is not running. Please start Docker Desktop." && exit 1)
	@echo "Docker is running ✓"

# Platform-specific builds
build-macos-native:
	@echo "Building for macOS (native)..."
	cargo build --release
	@mkdir -p target/aarch64-apple-darwin/release
	@cp target/release/fhir-fuse target/aarch64-apple-darwin/release/fhir-fuse
	@echo "Binary: target/aarch64-apple-darwin/release/fhir-fuse"

# Cross-compilation targets using Docker directly (works with Homebrew Rust)
build-linux-x64:
	@echo "Building for Linux x86_64 (using Docker)..."
	@mkdir -p target/x86_64-unknown-linux-gnu/release
	docker run --rm --platform linux/amd64 \
		-v "$(PWD)":/workspace \
		-v cargo-cache-x64:/usr/local/cargo/registry \
		-v cargo-git-cache-x64:/usr/local/cargo/git \
		-w /workspace \
		rust:latest \
		sh -c "apt-get update && apt-get install -y libfuse-dev pkg-config && cargo build --release && cp target/release/fhir-fuse target/x86_64-unknown-linux-gnu/release/"
	@echo "Binary: target/x86_64-unknown-linux-gnu/release/fhir-fuse"

build-linux-arm64:
	@echo "Building for Linux ARM64 (using Docker)..."
	@mkdir -p target/aarch64-unknown-linux-gnu/release
	docker run --rm --platform linux/arm64 \
		-v "$(PWD)":/workspace \
		-v cargo-cache-arm64:/usr/local/cargo/registry \
		-v cargo-git-cache-arm64:/usr/local/cargo/git \
		-w /workspace \
		rust:latest \
		sh -c "apt-get update && apt-get install -y libfuse-dev pkg-config && cargo build --release && cp target/release/fhir-fuse target/aarch64-unknown-linux-gnu/release/"
	@echo "Binary: target/aarch64-unknown-linux-gnu/release/fhir-fuse"

build-linux-musl-x64:
	@echo "Building for Linux x86_64 musl (Alpine Linux compatible) (using Docker)..."
	@mkdir -p target/x86_64-unknown-linux-musl/release
	docker run --rm --platform linux/amd64 \
		-v "$(PWD)":/workspace \
		-v cargo-cache-musl-x64:/usr/local/cargo/registry \
		-v cargo-git-cache-musl-x64:/usr/local/cargo/git \
		-w /workspace \
		rust:alpine \
		sh -c "apk add --no-cache fuse-dev fuse-static pkgconfig && cargo build --release && cp target/release/fhir-fuse target/x86_64-unknown-linux-musl/release/"
	@echo "Binary: target/x86_64-unknown-linux-musl/release/fhir-fuse"

build-linux-musl-arm64:
	@echo "Building for Linux ARM64 musl (Alpine Linux compatible) (using Docker)..."
	@mkdir -p target/aarch64-unknown-linux-musl/release
	docker run --rm --platform linux/arm64 \
		-v "$(PWD)":/workspace \
		-v cargo-cache-musl-arm64:/usr/local/cargo/registry \
		-v cargo-git-cache-musl-arm64:/usr/local/cargo/git \
		-w /workspace \
		rust:alpine \
		sh -c "apk add --no-cache fuse-dev fuse-static pkgconfig && cargo build --release && cp target/release/fhir-fuse target/aarch64-unknown-linux-musl/release/"
	@echo "Binary: target/aarch64-unknown-linux-musl/release/fhir-fuse"

# Aliases for Alpine Linux
build-alpine-x64: build-linux-musl-x64

build-alpine-arm64: build-linux-musl-arm64

build-windows-x64:
	@echo "⚠️  Windows builds are not supported for FUSE-based applications."
	@echo "FUSE (Filesystem in Userspace) is not available on Windows."
	@echo "Consider using WinFsp (Windows File System Proxy) as an alternative."

# Build for all platforms (excluding Windows as FUSE is not supported)
build-all: build-macos-native build-linux-x64 build-linux-arm64 build-linux-musl-x64 build-linux-musl-arm64
	@echo ""
	@echo "✅ All platform builds completed!"
	@echo ""
	@echo "Built binaries:"
	@echo "  macOS ARM64:           target/aarch64-apple-darwin/release/fhir-fuse"
	@echo "  Linux x86_64 (glibc):  target/x86_64-unknown-linux-gnu/release/fhir-fuse"
	@echo "  Linux ARM64 (glibc):   target/aarch64-unknown-linux-gnu/release/fhir-fuse"
	@echo "  Alpine x86_64 (musl):  target/x86_64-unknown-linux-musl/release/fhir-fuse"
	@echo "  Alpine ARM64 (musl):   target/aarch64-unknown-linux-musl/release/fhir-fuse"
	@echo ""
	@echo "Note: Windows builds are not supported (FUSE is not available on Windows)"

# Cache management targets
list-cache:
	@echo "Docker volume caches for Cargo builds:"
	@docker volume ls | grep "cargo-cache" || echo "No caches found"

cache-stats:
	@echo "Cache sizes:"
	@echo ""
	@echo "x86_64 (glibc) caches:"
	@docker volume inspect cargo-cache-x64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-cache-x64: not created yet"
	@docker volume inspect cargo-git-cache-x64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-git-cache-x64: not created yet"
	@echo ""
	@echo "ARM64 (glibc) caches:"
	@docker volume inspect cargo-cache-arm64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-cache-arm64: not created yet"
	@docker volume inspect cargo-git-cache-arm64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-git-cache-arm64: not created yet"
	@echo ""
	@echo "x86_64 musl (Alpine) caches:"
	@docker volume inspect cargo-cache-musl-x64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-cache-musl-x64: not created yet"
	@docker volume inspect cargo-git-cache-musl-x64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-git-cache-musl-x64: not created yet"
	@echo ""
	@echo "ARM64 musl (Alpine) caches:"
	@docker volume inspect cargo-cache-musl-arm64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-cache-musl-arm64: not created yet"
	@docker volume inspect cargo-git-cache-musl-arm64 2>/dev/null | grep -A1 Mountpoint || echo "  cargo-git-cache-musl-arm64: not created yet"

clean-cache:
	@echo "⚠️  This will remove all Cargo Docker volume caches!"
	@echo "Next builds will need to download and compile everything from scratch."
	@read -p "Are you sure? [y/N] " -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		echo "Removing caches..."; \
		docker volume rm cargo-cache-x64 cargo-git-cache-x64 2>/dev/null || true; \
		docker volume rm cargo-cache-arm64 cargo-git-cache-arm64 2>/dev/null || true; \
		docker volume rm cargo-cache-musl-x64 cargo-git-cache-musl-x64 2>/dev/null || true; \
		docker volume rm cargo-cache-musl-arm64 cargo-git-cache-musl-arm64 2>/dev/null || true; \
		echo "✅ Caches removed"; \
	else \
		echo "Cancelled"; \
	fi

# Docker image management
docker-build-arm64: build-linux-musl-arm64
	@echo "Building Docker image for ARM64 Linux..."
	docker build \
		-t ryukzak/fhir-fuse:arm64 \
		-t ryukzak/fhir-fuse:latest-arm64 \
		-f Dockerfile \
		.
	@echo "✅ Docker image built: ryukzak/fhir-fuse:arm64"

docker-push-arm64: docker-build-arm64
	@echo "Pushing Docker image for ARM64 Linux to registry..."
	docker push ryukzak/fhir-fuse:arm64
	docker push ryukzak/fhir-fuse:latest-arm64
	@echo "✅ Docker image pushed: ryukzak/fhir-fuse:arm64"
