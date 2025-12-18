# Quick Reference Card

## Starting FHIR-FUSE with Docker

### Quick Start (Recommended)

```bash
./docker-run.sh -d
```

That's it! Files will be available in `./mnt/Patient/`

### Manual Start

```bash
# Set architecture
export TARGETARCH=aarch64  # or x86_64

# Start services
docker-compose up -d

# Access files
ls ./mnt/Patient/
```

## Accessing Files

### On Host Machine

```bash
# List patients
ls ./mnt/Patient/

# View a patient
cat ./mnt/Patient/patient-123.json

# Pretty print with jq
jq . ./mnt/Patient/patient-123.json

# Search across all patients
grep -r "John" ./mnt/Patient/

# Count patients
ls ./mnt/Patient/ | wc -l
```

### Inside Container

```bash
# Access container
docker exec -it fhir-fuse-fhir-fuse-1 sh

# List patients
ls /mnt/fhir/Patient/

# View a patient
cat /mnt/fhir/Patient/patient-123.json
```

## Common Commands

### Check Status

```bash
# Check if services are running
docker-compose ps

# Check container logs
docker-compose logs fhir-fuse

# Check if FUSE is mounted
docker exec fhir-fuse-fhir-fuse-1 df -h /mnt/fhir
```

### Stop Services

```bash
# Stop all services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

### Restart Services

```bash
# Restart just fhir-fuse
docker-compose restart fhir-fuse

# Restart all services
docker-compose restart
```

## Troubleshooting

### Files Not Visible on Host

```bash
# 1. Check if files exist in container
docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir

# 2. Check bind propagation (should show "rshared")
docker inspect fhir-fuse-fhir-fuse-1 | grep -A5 Propagation

# 3. On Linux, make mount point shared
sudo mount --make-rshared ./mnt
docker-compose restart fhir-fuse
```

### Container Won't Start

```bash
# Check logs
docker-compose logs fhir-fuse

# Check if /dev/fuse exists
ls -l /dev/fuse

# If you see "transport endpoint is not connected"
./cleanup-mount.sh

# Rebuild container
docker-compose build --no-cache fhir-fuse
docker-compose up -d
```

### Permission Denied

```bash
# Check container is privileged
docker inspect fhir-fuse-fhir-fuse-1 | grep Privileged

# Should show: "Privileged": true
```

## Building

### Build for Current Platform

```bash
cargo build --release
```

### Build for All Platforms (with caching)

```bash
# First build: ~8 minutes
make build-all

# Subsequent builds: ~45 seconds
make build-all
```

### Build Specific Platform

```bash
make build-alpine-arm64    # Alpine ARM64
make build-alpine-x64      # Alpine x86_64
make build-linux-arm64     # Linux ARM64 (glibc)
make build-linux-x64       # Linux x86_64 (glibc)
```

## Cache Management

```bash
# List caches
make list-cache

# Show cache info
make cache-stats

# Clean all caches
make clean-cache
```

## Environment Variables

```bash
# Set architecture
export TARGETARCH=aarch64  # or x86_64

# Set custom mount directory
export MOUNT_DIR=/path/to/mount

# Use in docker-compose
docker-compose up -d
```

## Configuration Files

- `docker-compose.yaml` - Service definitions
- `Dockerfile` - Container image
- `Cargo.toml` - Rust dependencies
- `src/main.rs` - Application code

## Documentation

- `README.md` - Project overview
- `USAGE.md` - Detailed usage guide
- `DOCKER.md` - Docker setup guide
- `MOUNT_PROPAGATION.md` - Mount propagation explained
- `CACHING.md` - Build caching guide
- `BUILD_NOTES.md` - Build troubleshooting

## Key Concepts

### Mount Propagation

The `rshared` bind propagation makes FUSE mounts inside the container visible on the host:

```yaml
volumes:
  - type: bind
    source: ./mnt
    target: /mnt/fhir
    bind:
      propagation: rshared  # Critical for host visibility
```

### Required Permissions

```yaml
privileged: true           # Full container privileges
cap_add:
  - SYS_ADMIN             # Mount capability
devices:
  - /dev/fuse             # FUSE device access
security_opt:
  - apparmor:unconfined   # Disable AppArmor
```

## Architecture Support

- ✅ x86_64 (Intel/AMD)
- ✅ aarch64/ARM64 (Apple Silicon, ARM servers)
- ✅ Linux (native Docker)
- ✅ macOS (Docker Desktop)
- ⚠️ Windows (limited FUSE support)

## URLs

- **Aidbox UI**: http://localhost:8080
- **FHIR API**: http://localhost:8080/fhir
- **Health Check**: http://localhost:8080/health

## Default Credentials

- **Username**: admin
- **Password**: admin

## File Locations

- **Host mount**: `./mnt/`
- **Container mount**: `/mnt/fhir/`
- **Binaries**: `target/*/release/fhir-fuse`
- **Logs**: `docker-compose logs fhir-fuse`

## Performance

### Build Times (with caching)

- First build: ~8 minutes
- Subsequent: ~45 seconds
- **Speedup**: 10x faster

### Runtime

- Startup: ~30 seconds (waits for Aidbox)
- File access: Near real-time
- Memory: ~50-100 MB

## Tips

1. **Use docker-run.sh** - Handles everything automatically
2. **Check logs first** - Most issues show up in logs
3. **Verify inside container** - If files are there, it's a propagation issue
4. **Keep caches** - Saves ~7 minutes per rebuild
5. **Monitor resources** - Use `docker stats` to check usage

## Quick Diagnostics

```bash
# One-liner to check everything
docker-compose ps && \
docker exec fhir-fuse-fhir-fuse-1 df -h /mnt/fhir && \
ls -la ./mnt/ && \
echo "✅ All checks passed!"
```

## Getting Help

1. Check logs: `docker-compose logs fhir-fuse`
2. Read documentation in this directory
3. Check GitHub issues
4. Verify Docker/FUSE installation

## Common Patterns

### Development Workflow

```bash
# 1. Start services
./docker-run.sh -d

# 2. Make code changes
vim src/main.rs

# 3. Rebuild and restart
cargo build --release
docker-compose restart fhir-fuse

# 4. Test
ls ./mnt/Patient/
```

### Production Deployment

```bash
# 1. Build for target platform
make build-alpine-x64

# 2. Build Docker image
docker-compose build fhir-fuse

# 3. Deploy
docker-compose up -d

# 4. Monitor
docker-compose logs -f fhir-fuse
```

## Safety

⚠️ **Warning**: The container runs with extensive privileges:
- Use in isolated environments
- Monitor access logs
- Consider alternatives for production
- Restrict network access

## Summary

✅ **Easy to start**: `./docker-run.sh -d`
✅ **Files on host**: `./mnt/Patient/`
✅ **Fast rebuilds**: Caching enabled
✅ **Well documented**: Multiple guides available
✅ **Cross-platform**: Works on Linux and macOS

For detailed information, see the specific documentation files!

