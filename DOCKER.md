# Docker Setup for FHIR-FUSE

## Overview

This document describes the Docker setup for running FHIR-FUSE in a containerized environment.

## Architecture

The Docker Compose setup includes three services:

1. **PostgreSQL** - Database backend for Aidbox
2. **Aidbox** - FHIR server
3. **FHIR-FUSE** - Alpine-based container running the FUSE filesystem

## FHIR-FUSE Container Details

### Base Image
- **Alpine Linux** - Minimal, secure Linux distribution

### Installed Packages
- `fuse3` - FUSE3 runtime libraries
- `fuse3-dev` - FUSE3 development headers (for compatibility)
- `ca-certificates` - SSL/TLS certificates for HTTPS connections
- `curl` - HTTP client for health checks and debugging

### Binary
The container uses pre-built musl binaries from the `target/` directory:
- `target/x86_64-unknown-linux-musl/release/fhir-fuse` (for x86_64)
- `target/aarch64-unknown-linux-musl/release/fhir-fuse` (for ARM64)

### Required Permissions
The container requires special privileges to mount FUSE filesystems:
- `privileged: true` - Full container privileges
- `cap_add: SYS_ADMIN` - Capability to perform mount operations
- `devices: /dev/fuse` - Access to FUSE device
- `security_opt: apparmor:unconfined` - Disable AppArmor restrictions

### Volume Mounting
The FUSE filesystem is mounted at `/mnt/fhir` inside the container and bind-mounted to `./mnt` on the host with `rshared` propagation to ensure the mount is visible on the host.

**Key Point:** The `bind-propagation=rshared` setting is crucial - it makes the FUSE mount inside the container visible on the host machine. Without this, files would only be accessible inside the container.

See [MOUNT_PROPAGATION.md](MOUNT_PROPAGATION.md) for detailed explanation.

## Usage

### Prerequisites
1. Build the musl binaries for your architecture:
   ```bash
   # For x86_64
   cargo build --release --target x86_64-unknown-linux-musl
   
   # For ARM64 (aarch64)
   cargo build --release --target aarch64-unknown-linux-musl
   ```

2. Ensure Docker and Docker Compose are installed

### Running with Auto-Detection

Use the provided helper script that automatically detects your architecture:

```bash
./docker-run.sh
```

Or run in detached mode:

```bash
./docker-run.sh -d
```

### Running Manually

Set the architecture environment variable and start the services:

```bash
# For x86_64
export TARGETARCH=x86_64
docker-compose up -d

# For ARM64
export TARGETARCH=aarch64
docker-compose up -d
```

### Accessing the Filesystem

Once running, the FHIR filesystem is available at `./mnt`:

```bash
# List patients
ls ./mnt/Patient

# View a patient
cat ./mnt/Patient/<patient-id>.json

# Pretty print with jq
jq . ./mnt/Patient/<patient-id>.json
```

### Stopping the Services

```bash
docker-compose down
```

## Configuration

### Environment Variables

The FHIR-FUSE container accepts the following environment variables:

- `FHIR_SERVER_URL` - URL of the FHIR server (default: `http://aidbox:8080`)
- `MOUNT_POINT` - Mount point inside the container (default: `/mnt/fhir`)

### Customizing the Command

The default command is:
```
/usr/local/bin/fhir-fuse http://aidbox:8080 /mnt/fhir
```

You can override this in `docker-compose.override.yml`:

```yaml
services:
  fhir-fuse:
    command: ["/usr/local/bin/fhir-fuse", "http://custom-fhir-server:8080", "/mnt/fhir"]
```

## Troubleshooting

### Transport Endpoint Not Connected

**Error:** `invalid mount config for type "bind": stat ./mnt: transport endpoint is not connected`

This is a stale FUSE mount from a previous run.

**Solution:**

```bash
./cleanup-mount.sh
./docker-run.sh -d
```

Or manually:
```bash
docker-compose down
umount ./mnt 2>/dev/null || diskutil unmount force ./mnt 2>/dev/null
rm -rf ./mnt && mkdir -p ./mnt
docker-compose up -d
```

**Note:** The `docker-run.sh` script now automatically detects and fixes this issue!

### Container Fails to Start

1. **Check if FUSE device exists:**
   ```bash
   ls -l /dev/fuse
   ```

2. **Check if binary exists:**
   ```bash
   ls -l target/x86_64-unknown-linux-musl/release/fhir-fuse
   ```

3. **Check container logs:**
   ```bash
   docker-compose logs fhir-fuse
   ```

### Permission Denied Errors

The container needs privileged mode to mount FUSE filesystems. Ensure:
- Docker has permission to run privileged containers
- `/dev/fuse` is accessible
- SELinux/AppArmor policies allow FUSE mounts

### Mount Not Visible on Host

**This is the most common issue!** The FUSE mount is inside the container but not visible on the host.

**Solutions:**

1. **Check bind propagation is set:**
   ```yaml
   volumes:
     - type: bind
       source: ./mnt
       target: /mnt/fhir
       bind:
         propagation: rshared  # This is critical!
   ```

2. **On Linux, make the mount point shared:**
   ```bash
   sudo mount --make-rshared ./mnt
   docker-compose restart fhir-fuse
   ```

3. **Verify propagation:**
   ```bash
   docker inspect fhir-fuse-fhir-fuse-1 | grep -A5 Propagation
   ```
   Should show: `"Propagation": "rshared"`

4. **Check files inside container first:**
   ```bash
   docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir
   ```
   If files are there but not on host, it's a propagation issue.

See [MOUNT_PROPAGATION.md](MOUNT_PROPAGATION.md) for complete troubleshooting guide.

### Architecture Mismatch

If you see "exec format error", you're using the wrong architecture binary. Set `TARGETARCH` correctly:
- `x86_64` for Intel/AMD processors
- `aarch64` for ARM64 processors (Apple Silicon, ARM servers)

## Security Considerations

⚠️ **Warning**: The FHIR-FUSE container runs in privileged mode, which gives it extensive access to the host system. This is necessary for FUSE filesystem mounting but should be used with caution in production environments.

Consider:
- Running in isolated networks
- Using read-only root filesystem where possible
- Implementing proper access controls on the FHIR server
- Regular security updates of the Alpine base image

## Building Custom Images

To build a custom image with different configurations:

```bash
docker build -t my-fhir-fuse --build-arg TARGETARCH=x86_64 .
```

## Files

- `Dockerfile` - Container definition
- `docker-compose.yaml` - Service orchestration
- `docker-run.sh` - Helper script with auto-detection
- `.dockerignore` - Build context optimization

