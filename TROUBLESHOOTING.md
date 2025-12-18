# Troubleshooting Guide

## Common Issues and Solutions

### 1. Transport Endpoint Not Connected

**Error Message:**
```
Error response from daemon: invalid mount config for type "bind": 
stat /Users/aitem/Work/fhir-fuse/mnt: transport endpoint is not connected
```

**Cause:** Stale FUSE mount from a previous run. The directory is in a bad state.

**Solution:**

**Quick Fix:**
```bash
./cleanup-mount.sh
./docker-run.sh -d
```

**Manual Fix:**
```bash
# Stop containers
docker-compose down

# Unmount the directory
umount ./mnt 2>/dev/null || diskutil unmount force ./mnt 2>/dev/null

# Recreate directory
rm -rf ./mnt
mkdir -p ./mnt

# Restart
docker-compose up -d
```

**Prevention:** The updated `docker-run.sh` now automatically detects and fixes stale mounts!

---

### 2. Files Not Visible on Host

**Problem:** Container shows files, but `./mnt` is empty on host.

**Diagnosis:**
```bash
# Check if files exist in container
docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir

# If files are there but not on host, it's a propagation issue
```

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

3. **Verify docker-compose.yaml has correct settings:**
   ```yaml
   volumes:
     - type: bind
       source: ./mnt
       target: /mnt/fhir
       bind:
         propagation: rshared  # Must be present!
   ```

---

### 3. Container Won't Start

**Error:** Container exits immediately after starting.

**Diagnosis:**
```bash
# Check logs
docker-compose logs fhir-fuse

# Common errors:
# - FUSE device not available
# - Permission denied
# - Cannot connect to FHIR server
```

**Solutions:**

1. **Check if /dev/fuse exists:**
   ```bash
   ls -l /dev/fuse
   ```
   If not found, install FUSE:
   ```bash
   # macOS
   brew install macfuse
   
   # Linux
   sudo apt-get install fuse3
   ```

2. **Check container is privileged:**
   ```bash
   docker inspect fhir-fuse-fhir-fuse-1 | grep Privileged
   ```
   Should show: `"Privileged": true`

3. **Wait for Aidbox to be ready:**
   ```bash
   # Check Aidbox health
   curl http://localhost:8080/health
   
   # Container waits for Aidbox health check
   docker-compose ps
   ```

4. **Check binary architecture matches:**
   ```bash
   # Verify TARGETARCH is set correctly
   echo $TARGETARCH
   
   # Should be aarch64 for ARM64 or x86_64 for Intel/AMD
   ```

---

### 4. Permission Denied

**Error:** Cannot access files in `./mnt`

**Solutions:**

1. **Check file permissions:**
   ```bash
   ls -la ./mnt/
   ```

2. **Check if FUSE is mounted:**
   ```bash
   docker exec fhir-fuse-fhir-fuse-1 mount | grep fuse
   ```

3. **Check container logs for errors:**
   ```bash
   docker-compose logs fhir-fuse | grep -i error
   ```

4. **Verify container has required capabilities:**
   ```bash
   docker inspect fhir-fuse-fhir-fuse-1 | grep -A5 CapAdd
   ```
   Should show: `"SYS_ADMIN"`

---

### 5. Cannot Connect to FHIR Server

**Error:** Container logs show connection errors to Aidbox.

**Solutions:**

1. **Check Aidbox is running:**
   ```bash
   docker-compose ps aidbox
   curl http://localhost:8080/health
   ```

2. **Check network connectivity:**
   ```bash
   docker exec fhir-fuse-fhir-fuse-1 ping -c 3 aidbox
   docker exec fhir-fuse-fhir-fuse-1 curl -v http://aidbox:8080/health
   ```

3. **Verify FHIR server URL in command:**
   ```yaml
   command: ["/usr/local/bin/fhir-fuse", "/mnt/fhir", "http://aidbox:8080/fhir"]
   ```

4. **Check depends_on is configured:**
   ```yaml
   depends_on:
     aidbox:
       condition: service_healthy
   ```

---

### 6. Build Fails for Alpine

**Error:** OpenSSL or FUSE linking errors during build.

**Solutions:**

1. **Verify Cargo.toml uses rustls:**
   ```toml
   reqwest = { version = "0.12", features = ["blocking", "json", "rustls-tls"], default-features = false }
   ```

2. **Check Makefile includes fuse-static:**
   ```bash
   # Should include: fuse-dev fuse-static pkgconfig
   grep "fuse-static" Makefile
   ```

3. **Rebuild from scratch:**
   ```bash
   make clean
   make build-alpine-arm64
   ```

See [BUILD_NOTES.md](BUILD_NOTES.md) for detailed build troubleshooting.

---

### 7. Slow Performance

**Problem:** File access is slow or container uses too much CPU/memory.

**Diagnosis:**
```bash
# Check resource usage
docker stats fhir-fuse-fhir-fuse-1

# Check number of files
docker exec fhir-fuse-fhir-fuse-1 find /mnt/fhir -type f | wc -l
```

**Solutions:**

1. **Limit resource usage:**
   ```yaml
   deploy:
     resources:
       limits:
         cpus: '1.0'
         memory: 512M
   ```

2. **Check FHIR server response time:**
   ```bash
   time curl http://localhost:8080/fhir/Patient
   ```

3. **Enable caching in application** (if implemented)

---

### 8. Docker Credential Errors

**Error:** `error getting credentials - err: exit status 1`

**Cause:** Docker credential helper issue (common on macOS).

**Solutions:**

1. **Try pulling image manually:**
   ```bash
   docker pull rust:alpine
   ```

2. **Reset Docker credentials:**
   ```bash
   rm ~/.docker/config.json
   docker login
   ```

3. **Use different credential store:**
   Edit `~/.docker/config.json`:
   ```json
   {
     "credsStore": ""
   }
   ```

---

### 9. Cache Not Working

**Problem:** Builds are still slow despite caching.

**Diagnosis:**
```bash
# Check if volumes exist
make list-cache

# Should show cargo-cache-* volumes
```

**Solutions:**

1. **Verify volumes are mounted:**
   ```bash
   docker inspect <container> | grep -A10 Mounts
   ```

2. **Check cache isn't full:**
   ```bash
   docker system df -v | grep cargo-cache
   ```

3. **Rebuild cache:**
   ```bash
   make clean-cache
   make build-alpine-arm64
   ```

See [CACHING.md](CACHING.md) for detailed cache troubleshooting.

---

### 10. macOS Specific Issues

#### FUSE Not Available

**Error:** `/dev/fuse: No such file or directory`

**Solution:**
```bash
# Install macFUSE
brew install macfuse

# Restart Docker Desktop
# May need to enable kernel extension in System Settings
```

#### Mount Not Visible

**Problem:** Files in container but not on host (macOS with Docker Desktop).

**Solution:**
1. Open Docker Desktop → Settings → Resources → File Sharing
2. Add your project directory: `/Users/aitem/Work/fhir-fuse`
3. Click "Apply & Restart"

---

## Diagnostic Commands

### Quick Health Check

```bash
# Run all checks
docker-compose ps && \
docker exec fhir-fuse-fhir-fuse-1 df -h /mnt/fhir && \
ls -la ./mnt/ && \
echo "✅ All checks passed!"
```

### Detailed Diagnostics

```bash
# 1. Check Docker is running
docker ps

# 2. Check services status
docker-compose ps

# 3. Check container logs
docker-compose logs fhir-fuse

# 4. Check FUSE mount inside container
docker exec fhir-fuse-fhir-fuse-1 mount | grep fuse

# 5. Check files inside container
docker exec fhir-fuse-fhir-fuse-1 ls -la /mnt/fhir

# 6. Check files on host
ls -la ./mnt

# 7. Check bind propagation
docker inspect fhir-fuse-fhir-fuse-1 | grep -A5 Propagation

# 8. Check container privileges
docker inspect fhir-fuse-fhir-fuse-1 | grep Privileged

# 9. Check Aidbox connectivity
docker exec fhir-fuse-fhir-fuse-1 curl -s http://aidbox:8080/health

# 10. Check resource usage
docker stats --no-stream fhir-fuse-fhir-fuse-1
```

---

## Getting Help

If you're still having issues:

1. **Check logs:**
   ```bash
   docker-compose logs fhir-fuse > logs.txt
   ```

2. **Gather diagnostics:**
   ```bash
   docker-compose ps > diagnostics.txt
   docker inspect fhir-fuse-fhir-fuse-1 >> diagnostics.txt
   mount | grep mnt >> diagnostics.txt
   ```

3. **Review documentation:**
   - [MOUNT_PROPAGATION.md](MOUNT_PROPAGATION.md) - Mount issues
   - [DOCKER.md](DOCKER.md) - Docker setup
   - [BUILD_NOTES.md](BUILD_NOTES.md) - Build issues
   - [CACHING.md](CACHING.md) - Cache issues

4. **Check GitHub issues** for similar problems

5. **Create new issue** with:
   - Error message
   - Output of diagnostic commands
   - OS and Docker version
   - Steps to reproduce

---

## Prevention Tips

1. **Always use cleanup script** when things go wrong:
   ```bash
   ./cleanup-mount.sh
   ```

2. **Use docker-run.sh** instead of manual docker-compose:
   ```bash
   ./docker-run.sh -d
   ```

3. **Stop cleanly:**
   ```bash
   docker-compose down  # Not Ctrl+C
   ```

4. **Monitor logs** during startup:
   ```bash
   docker-compose up  # Without -d to see logs
   ```

5. **Keep Docker updated:**
   ```bash
   docker --version
   docker-compose --version
   ```

---

## Quick Fixes Summary

| Problem | Quick Fix |
|---------|-----------|
| Transport endpoint not connected | `./cleanup-mount.sh` |
| Files not visible on host | Check propagation: `docker inspect ... \| grep Propagation` |
| Container won't start | Check logs: `docker-compose logs fhir-fuse` |
| Permission denied | Check privileged: `docker inspect ... \| grep Privileged` |
| Can't connect to FHIR | Check Aidbox: `curl http://localhost:8080/health` |
| Build fails | Use rustls: Check `Cargo.toml` |
| Slow performance | Check stats: `docker stats` |
| Credential errors | Reset: `rm ~/.docker/config.json` |
| Cache not working | List: `make list-cache` |
| macOS FUSE issues | Install: `brew install macfuse` |

---

## Still Stuck?

Try the nuclear option:

```bash
# Stop everything
docker-compose down -v

# Clean mount
./cleanup-mount.sh

# Clean Docker
docker system prune -f

# Rebuild
docker-compose build --no-cache

# Start fresh
./docker-run.sh -d
```

This solves 90% of issues by starting completely fresh!

