# FUSE Mount Propagation Guide

## Overview

This guide explains how to make FUSE mounts inside Docker containers visible on the host machine using bind-propagation.

## The Challenge

When you mount a FUSE filesystem inside a Docker container, by default it's **only visible inside the container**. The host machine cannot see the mounted files.

```
┌─────────────────────────────────────────┐
│           Host Machine                   │
│                                          │
│  ./mnt/  ← Empty directory              │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │     Docker Container                │ │
│  │                                     │ │
│  │  /mnt/fhir/  ← FUSE mounted here   │ │
│  │  ├── Patient/                      │ │
│  │  │   ├── patient-1.json            │ │
│  │  │   └── patient-2.json            │ │
│  │                                     │ │
│  └────────────────────────────────────┘ │
│                                          │
│  ❌ Files NOT visible on host!          │
└─────────────────────────────────────────┘
```

## The Solution: Bind Propagation

Using `bind-propagation=rshared`, mounts inside the container become visible on the host:

```
┌─────────────────────────────────────────┐
│           Host Machine                   │
│                                          │
│  ./mnt/  ← FUSE mount visible here! ✓   │
│  ├── Patient/                           │
│  │   ├── patient-1.json                 │
│  │   └── patient-2.json                 │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │     Docker Container                │ │
│  │                                     │ │
│  │  /mnt/fhir/  ← FUSE mounted here   │ │
│  │  ├── Patient/                      │ │
│  │  │   ├── patient-1.json            │ │
│  │  │   └── patient-2.json            │ │
│  │                                     │ │
│  └────────────────────────────────────┘ │
│                                          │
│  ✅ Files visible on host via ./mnt/    │
└─────────────────────────────────────────┘
```

## Docker Compose Configuration

### Key Settings

```yaml
fhir-fuse:
  privileged: true              # Required for FUSE
  cap_add:
    - SYS_ADMIN                 # Required for mounting
  devices:
    - /dev/fuse                 # FUSE device access
  security_opt:
    - apparmor:unconfined       # Disable AppArmor restrictions
  volumes:
    - type: bind
      source: ${MOUNT_DIR:-./mnt}
      target: /mnt/fhir
      bind:
        propagation: rshared    # KEY: Makes mounts visible on host
```

### Comparison with Your Example

Your example:
```bash
docker run --rm \
  --mount type=bind,source=/s3ql,target=/s3ql,bind-propagation=rshared \
  --cap-add SYS_ADMIN --device /dev/fuse --name myContainer \
  myS3qlImage mount.s3ql swiftks://url:container /s3ql
```

Our docker-compose equivalent:
```yaml
volumes:
  - type: bind
    source: ${MOUNT_DIR:-./mnt}    # Host directory
    target: /mnt/fhir               # Container directory
    bind:
      propagation: rshared          # Same as bind-propagation=rshared
```

## Usage

### Option 1: Using docker-run.sh (Recommended)

The script handles everything automatically:

```bash
./docker-run.sh -d
```

On Linux, it will:
1. Detect your architecture
2. Create the mount directory
3. Set up mount propagation on the host
4. Start docker-compose

### Option 2: Manual Setup

#### On Linux:

```bash
# 1. Set architecture
export TARGETARCH=aarch64  # or x86_64

# 2. Create mount directory
mkdir -p ./mnt

# 3. Make the directory shared (important!)
sudo mount --make-rshared ./mnt

# 4. Start docker-compose
docker-compose up -d

# 5. Access files on host
ls ./mnt/Patient
```

#### On macOS:

```bash
# 1. Set architecture
export TARGETARCH=aarch64  # or x86_64

# 2. Create mount directory
mkdir -p ./mnt

# 3. Start docker-compose
docker-compose up -d

# 4. Access files on host
ls ./mnt/Patient
```

**Note:** macOS handles mount propagation differently through Docker Desktop's VM layer.

### Option 3: Custom Mount Directory

You can specify a different mount directory:

```bash
export MOUNT_DIR=/path/to/custom/mount
./docker-run.sh -d
```

## Verification

### Check if FUSE is mounted inside container:

```bash
docker exec -it fhir-fuse-fhir-fuse-1 df -h /mnt/fhir
```

Expected output:
```
Filesystem      Size  Used Avail Use% Mounted on
fhir-fuse       1.0G     0  1.0G   0% /mnt/fhir
```

### Check if files are visible on host:

```bash
ls -la ./mnt/
```

Expected output:
```
drwxr-xr-x  3 user  group   96 Dec 18 10:00 .
drwxr-xr-x 15 user  group  480 Dec 18 09:55 ..
drwxr-xr-x  2 user  group   64 Dec 18 10:00 Patient
```

### Check mount propagation (Linux only):

```bash
findmnt -o TARGET,PROPAGATION ./mnt
```

Expected output:
```
TARGET PROPAGATION
./mnt  shared
```

## Troubleshooting

### Files Not Visible on Host

**Problem:** Container shows files, but host directory is empty.

**Solutions:**

1. **Check bind propagation:**
   ```bash
   docker inspect fhir-fuse-fhir-fuse-1 | grep -A5 Propagation
   ```
   Should show: `"Propagation": "rshared"`

2. **On Linux, make mount point shared:**
   ```bash
   sudo mount --make-rshared ./mnt
   docker-compose restart fhir-fuse
   ```

3. **Check container privileges:**
   ```bash
   docker inspect fhir-fuse-fhir-fuse-1 | grep Privileged
   ```
   Should show: `"Privileged": true`

### Permission Denied

**Problem:** Cannot access files on host.

**Solutions:**

1. **Check file permissions:**
   ```bash
   ls -la ./mnt/
   ```

2. **Check if FUSE is actually mounted:**
   ```bash
   docker exec fhir-fuse-fhir-fuse-1 mount | grep fuse
   ```

3. **Check container logs:**
   ```bash
   docker-compose logs fhir-fuse
   ```

### Mount Point Busy / Transport Endpoint Not Connected

**Problem:** Error like "transport endpoint is not connected" or "invalid mount config".

This happens when there's a stale FUSE mount from a previous run.

**Quick Fix:**

```bash
./cleanup-mount.sh
```

**Manual Fix:**

1. **Stop containers:**
   ```bash
   docker-compose down
   ```

2. **Unmount on host:**
   ```bash
   # macOS
   umount ./mnt
   diskutil unmount force ./mnt
   
   # Linux
   fusermount -u ./mnt
   sudo umount ./mnt
   ```

3. **Recreate directory:**
   ```bash
   rm -rf ./mnt
   mkdir -p ./mnt
   ```

4. **Restart:**
   ```bash
   ./docker-run.sh -d
   ```

**Note:** The updated `docker-run.sh` now automatically detects and fixes stale mounts!

### macOS Specific Issues

**Problem:** Mount propagation not working on macOS.

**Note:** macOS with Docker Desktop uses a Linux VM, so mount propagation works differently:

1. Mounts inside containers are visible through Docker Desktop's VM layer
2. The `rshared` propagation is handled by Docker Desktop
3. You may need to add `./mnt` to Docker Desktop's file sharing settings

**Solution:**
- Open Docker Desktop → Settings → Resources → File Sharing
- Add your project directory
- Restart Docker Desktop

## Platform Differences

### Linux (Native Docker)

```
Host ← rshared → Container
     (direct mount propagation)
```

- Direct mount propagation
- Requires `sudo mount --make-rshared`
- Most reliable for FUSE mounts

### macOS (Docker Desktop)

```
Host ← Docker VM ← rshared → Container
     (via VM layer)
```

- Indirect through VM
- Handled by Docker Desktop
- May have slight delays
- Requires file sharing configured

### Windows (Docker Desktop + WSL2)

```
Host ← WSL2 ← Docker VM ← rshared → Container
     (via multiple layers)
```

- Multiple translation layers
- FUSE support limited
- Consider using WSL2 directly

## Best Practices

1. **Use docker-run.sh** - Handles platform differences automatically

2. **Check logs** - If files aren't visible:
   ```bash
   docker-compose logs fhir-fuse
   ```

3. **Verify mount** - Inside container:
   ```bash
   docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir
   ```

4. **Clean shutdown** - Always stop properly:
   ```bash
   docker-compose down
   ```

5. **Monitor resources** - FUSE can be resource-intensive:
   ```bash
   docker stats fhir-fuse-fhir-fuse-1
   ```

## Advanced Configuration

### Custom Mount Options

You can pass additional FUSE options via environment variables:

```yaml
environment:
  FUSE_OPTIONS: "-o allow_other,default_permissions"
```

### Multiple Mount Points

To mount to multiple locations:

```yaml
volumes:
  - type: bind
    source: ./mnt1
    target: /mnt/fhir1
    bind:
      propagation: rshared
  - type: bind
    source: ./mnt2
    target: /mnt/fhir2
    bind:
      propagation: rshared
```

### Read-Only Host Access

If you want the host to see files but not modify them:

```yaml
volumes:
  - type: bind
    source: ./mnt
    target: /mnt/fhir
    read_only: false  # Container needs write access for FUSE
    bind:
      propagation: rshared
```

Then on host, mount read-only:
```bash
sudo mount --bind -o ro ./mnt ./mnt-readonly
```

## Security Considerations

⚠️ **Warning:** The container runs with extensive privileges:

- `privileged: true` - Full host access
- `SYS_ADMIN` capability - Can perform mounts
- `/dev/fuse` access - Can create FUSE filesystems
- `apparmor:unconfined` - No AppArmor restrictions

**Recommendations:**

1. **Isolate network** - Use separate Docker network
2. **Limit resources** - Set memory/CPU limits
3. **Monitor access** - Check logs regularly
4. **Use in development** - Consider alternatives for production
5. **Restrict mount points** - Only mount what's needed

## Summary

✅ **Bind propagation** (`rshared`) makes FUSE mounts visible on host
✅ **Privileged mode** required for FUSE operations
✅ **Platform differences** handled by docker-run.sh
✅ **Easy access** - Files appear in `./mnt` on host
✅ **Automatic setup** - Just run `./docker-run.sh -d`

The configuration matches your S3QL example and enables seamless FUSE mount sharing between container and host!

