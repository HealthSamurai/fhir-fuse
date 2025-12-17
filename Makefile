.PHONY: help build run release clean test check fmt lint install-targets build-all

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
		-w /workspace \
		rust:latest \
		sh -c "apt-get update && apt-get install -y libfuse-dev pkg-config && cargo build --release && cp target/release/fhir-fuse target/x86_64-unknown-linux-gnu/release/"
	@echo "Binary: target/x86_64-unknown-linux-gnu/release/fhir-fuse"

build-linux-arm64:
	@echo "Building for Linux ARM64 (using Docker)..."
	@mkdir -p target/aarch64-unknown-linux-gnu/release
	docker run --rm --platform linux/arm64 \
		-v "$(PWD)":/workspace \
		-w /workspace \
		rust:latest \
		sh -c "apt-get update && apt-get install -y libfuse-dev pkg-config && cargo build --release && cp target/release/fhir-fuse target/aarch64-unknown-linux-gnu/release/"
	@echo "Binary: target/aarch64-unknown-linux-gnu/release/fhir-fuse"

build-linux-musl-x64:
	@echo "Building for Linux x86_64 musl (Alpine Linux compatible) (using Docker)..."
	@mkdir -p target/x86_64-unknown-linux-musl/release
	docker run --rm --platform linux/amd64 \
		-v "$(PWD)":/workspace \
		-w /workspace \
		rust:alpine \
		sh -c "apk add --no-cache fuse-dev pkgconfig && cargo build --release && cp target/release/fhir-fuse target/x86_64-unknown-linux-musl/release/"
	@echo "Binary: target/x86_64-unknown-linux-musl/release/fhir-fuse"

build-linux-musl-arm64:
	@echo "Building for Linux ARM64 musl (Alpine Linux compatible) (using Docker)..."
	@mkdir -p target/aarch64-unknown-linux-musl/release
	docker run --rm --platform linux/arm64 \
		-v "$(PWD)":/workspace \
		-w /workspace \
		rust:alpine \
		sh -c "apk add --no-cache fuse-dev pkgconfig && cargo build --release && cp target/release/fhir-fuse target/aarch64-unknown-linux-musl/release/"
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

