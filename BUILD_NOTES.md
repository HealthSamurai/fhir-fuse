# Build Notes for Alpine Linux

## Problem

When building for Alpine Linux (musl target), the build was failing with two main issues:

1. **OpenSSL dependency**: The `reqwest` crate was trying to use OpenSSL, which requires complex cross-compilation setup for musl targets
2. **Missing static FUSE library**: The linker couldn't find the static version of libfuse

## Solution

### 1. Switch to rustls-tls

Modified `Cargo.toml` to use `rustls-tls` instead of the default OpenSSL backend:

```toml
reqwest = { version = "0.12", features = ["blocking", "json", "rustls-tls"], default-features = false }
```

**Benefits:**
- Pure Rust implementation (no C dependencies)
- Better cross-compilation support
- Smaller binary size
- Easier to statically link

### 2. Install Static FUSE Library

Updated the Makefile to install `fuse-static` package in addition to `fuse-dev`:

```bash
apk add --no-cache fuse-dev fuse-static pkgconfig
```

This provides the static version of libfuse (`libfuse.a`) required for static linking with musl.

## Build Results

### ARM64 (aarch64) Alpine Binary

```bash
make build-alpine-arm64
```

**Output:**
- File: `target/aarch64-unknown-linux-musl/release/fhir-fuse`
- Size: 4.9 MB
- Type: ELF 64-bit LSB executable, ARM aarch64, **statically linked**
- Status: ✅ Successfully built

### x86_64 Alpine Binary

```bash
make build-alpine-x64
```

**Note:** May require Docker credential configuration, but the build process is identical to ARM64.

## Binary Characteristics

The resulting binaries are:
- ✅ **Statically linked** - No runtime dependencies
- ✅ **musl libc** - Compatible with Alpine Linux
- ✅ **rustls-tls** - Pure Rust TLS implementation
- ✅ **Optimized** - Release build with optimizations
- ✅ **Stripped** - Debug symbols removed for smaller size

## Verification

To verify the binary is properly built:

```bash
# Check file type
file target/aarch64-unknown-linux-musl/release/fhir-fuse

# Expected output:
# ELF 64-bit LSB executable, ARM aarch64, version 1 (SYSV), statically linked

# Check size
ls -lh target/aarch64-unknown-linux-musl/release/fhir-fuse
```

## Docker Integration

The Dockerfile uses these pre-built binaries:

```dockerfile
ARG TARGETARCH
COPY target/${TARGETARCH}-unknown-linux-musl/release/fhir-fuse /usr/local/bin/fhir-fuse
```

The Alpine container only needs runtime FUSE packages:
- `fuse3` - FUSE3 runtime
- `fuse3-dev` - FUSE3 headers (for compatibility)
- `ca-certificates` - SSL certificates
- `curl` - HTTP client

## Dependencies Summary

### Build-time (in rust:alpine container)
- `fuse-dev` - FUSE development headers
- `fuse-static` - Static FUSE library
- `pkgconfig` - Package configuration tool

### Runtime (in Alpine container)
- `fuse3` - FUSE3 runtime libraries
- `fuse3-dev` - FUSE3 development headers
- `ca-certificates` - SSL/TLS certificates
- `curl` - HTTP client

## Troubleshooting

### Error: "cannot find -lfuse"

**Solution:** Install `fuse-static` package
```bash
apk add fuse-static
```

### Error: "Could not find openssl via pkg-config"

**Solution:** Use rustls instead of OpenSSL
```toml
reqwest = { version = "0.12", features = ["rustls-tls"], default-features = false }
```

### Error: "exec format error" in Docker

**Solution:** Ensure TARGETARCH matches your platform
- Use `x86_64` for Intel/AMD
- Use `aarch64` for ARM64

## Performance Notes

The statically linked binary is larger (~5MB) compared to dynamically linked versions (~450KB) because it includes:
- All Rust standard library code
- FUSE library code
- rustls and crypto libraries
- HTTP client code

However, this is a reasonable tradeoff for:
- Zero runtime dependencies
- Easier deployment
- Better portability
- Consistent behavior across Alpine versions

