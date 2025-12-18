# macOS Limitations and Workarounds

## The Problem

On **macOS with Docker Desktop**, FUSE mounts inside containers **cannot be propagated to the host filesystem**. This is a fundamental limitation due to how Docker Desktop works on macOS.

### Why This Happens

```
┌─────────────────────────────────────────────────────────────┐
│                    macOS Host                                │
│                                                               │
│  ./mnt/  ← Cannot see FUSE mount ❌                          │
│                                                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │            Docker Desktop VM (Linux)                   │  │
│  │                                                        │  │
│  │  ┌──────────────────────────────────────────────────┐ │  │
│  │  │        Docker Container                           │ │  │
│  │  │                                                   │ │  │
│  │  │  /mnt/fhir/  ← FUSE mounted here ✓               │ │  │
│  │  │  ├── Patient/                                     │ │  │
│  │  │  │   ├── patient-1.json                           │ │  │
│  │  │  │   └── patient-2.json                           │ │  │
│  │  └──────────────────────────────────────────────────┘ │  │
│  │                                                        │  │
│  │  The VM can see the mount, but it can't propagate     │  │
│  │  back through the VM layer to macOS                   │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### Technical Details

1. **Docker Desktop on macOS** runs Linux containers inside a lightweight VM
2. **FUSE mounts** happen inside the Linux VM
3. **Mount propagation** (`rshared`) works within Linux, but not across the VM boundary
4. **macOS filesystem** cannot see mounts inside the VM

This is different from:
- **Linux**: Direct Docker, mount propagation works perfectly ✅
- **Windows WSL2**: Similar VM limitation ❌

## Workarounds

### Option 1: Use access-fuse.sh Script (Recommended)

The `access-fuse.sh` script provides easy access to files inside the container:

```bash
# List all patients
./access-fuse.sh ls Patient

# View a patient file
./access-fuse.sh cat Patient/patient-1.json

# Pretty print with jq
./access-fuse.sh cat Patient/patient-1.json | jq .

# Find files
./access-fuse.sh find '*.json'

# Copy a file to host
./access-fuse.sh copy Patient/patient-1.json ./patient-1.json

# Sync all files to ./mnt
./access-fuse.sh sync

# Open shell in container
./access-fuse.sh shell
```

### Option 2: Docker Exec Commands

Access files directly using docker exec:

```bash
# List patients
docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir/Patient

# View a patient
docker exec fhir-fuse-fhir-fuse-1 cat /mnt/fhir/Patient/patient-1.json

# Copy file to host
docker exec fhir-fuse-fhir-fuse-1 cat /mnt/fhir/Patient/patient-1.json > patient-1.json

# Interactive shell
docker exec -it fhir-fuse-fhir-fuse-1 sh
cd /mnt/fhir
ls Patient/
```

### Option 3: Sync Files to Host

Copy all files from container to host:

```bash
# One-time sync
docker exec fhir-fuse-fhir-fuse-1 tar -C /mnt/fhir -cf - . | tar -C ./mnt -xf -

# Or use the script
./access-fuse.sh sync

# Now you can access files in ./mnt
ls ./mnt/Patient/
cat ./mnt/Patient/patient-1.json
```

**Note:** Files are copied, not live-mounted. Changes in FHIR server won't automatically appear.

### Option 4: Docker Volume Mount (Alternative Architecture)

Instead of FUSE, use a sidecar container that periodically syncs data:

```yaml
services:
  fhir-sync:
    image: alpine
    volumes:
      - ./mnt:/data
    command: |
      sh -c "while true; do
        wget -O /data/patients.json http://aidbox:8080/fhir/Patient
        sleep 60
      done"
```

This doesn't use FUSE but provides host access to data.

### Option 5: Run Natively on macOS

For development, run fhir-fuse natively on macOS:

```bash
# Install macFUSE
brew install macfuse

# Build native binary
cargo build --release

# Run locally
./target/release/fhir-fuse /tmp/fhir http://localhost:8080/fhir

# Access files on host
ls /tmp/fhir/Patient/
```

**Pros:**
- ✅ Direct host filesystem access
- ✅ No Docker limitations
- ✅ Better performance

**Cons:**
- ❌ Requires macFUSE installation
- ❌ May need kernel extension approval
- ❌ Less isolated than Docker

### Option 6: Use Linux VM or Remote Server

For production or testing mount propagation:

1. **Use a Linux VM** (UTM, Parallels, VirtualBox)
2. **Use a remote Linux server**
3. **Use GitHub Codespaces** (Linux environment)
4. **Use AWS Cloud9** or similar cloud IDE

On Linux, mount propagation works perfectly:

```bash
# On Linux
./docker-run.sh -d
ls ./mnt/Patient/  # Files visible! ✅
```

## Comparison of Options

| Option | Host Access | Real-time | Complexity | Best For |
|--------|-------------|-----------|------------|----------|
| access-fuse.sh | Via script | Yes | Low | Quick access |
| Docker exec | Via commands | Yes | Low | Automation |
| Sync files | Direct | No | Low | One-time export |
| Sidecar sync | Direct | Periodic | Medium | Simple polling |
| Native macOS | Direct | Yes | Medium | Development |
| Linux VM | Direct | Yes | High | Production testing |

## Recommended Workflow for macOS

### For Development

```bash
# Option A: Use native binary
brew install macfuse
cargo build --release
./target/release/fhir-fuse /tmp/fhir http://localhost:8080/fhir

# Option B: Use access script with Docker
./docker-run.sh -d
./access-fuse.sh ls Patient
./access-fuse.sh cat Patient/patient-1.json | jq .
```

### For Testing

```bash
# Use Docker with access script
./docker-run.sh -d

# Access files as needed
./access-fuse.sh shell
# or
./access-fuse.sh sync  # Copy all to ./mnt
```

### For Production

Deploy on Linux where mount propagation works natively:

```bash
# On Linux server
./docker-run.sh -d
ls ./mnt/Patient/  # Works perfectly! ✅
```

## Why Not Just Fix It?

Unfortunately, this limitation cannot be "fixed" because:

1. **Docker Desktop architecture** - The VM layer is fundamental to how Docker Desktop works on macOS
2. **macOS kernel** - macOS doesn't support Linux kernel features like mount namespaces
3. **FUSE on macOS** - macFUSE works differently than Linux FUSE
4. **Security model** - macOS has stricter security around kernel extensions

## Future Possibilities

Potential solutions that don't exist yet:

1. **Docker Desktop enhancement** - Could theoretically sync FUSE mounts through the VM
2. **macFUSE integration** - Could bridge Linux FUSE to macOS FUSE
3. **Native macOS containers** - If macOS supported native containers (unlikely)

For now, the workarounds above are the best options.

## Quick Reference

### I want to...

**View files quickly:**
```bash
./access-fuse.sh ls Patient
./access-fuse.sh cat Patient/patient-1.json
```

**Work with files on host:**
```bash
./access-fuse.sh sync
# Files now in ./mnt
```

**Develop interactively:**
```bash
# Run natively
brew install macfuse
cargo run -- /tmp/fhir http://localhost:8080/fhir
```

**Test on Linux:**
```bash
# Use Linux VM or server
./docker-run.sh -d
ls ./mnt/Patient/  # Works! ✅
```

## Summary

✅ **FUSE works** inside the Docker container
❌ **Mount propagation doesn't work** from container to macOS host
✅ **Workarounds available** via access script or native execution
✅ **Works perfectly on Linux** for production deployment

The `access-fuse.sh` script makes it easy to work with files despite the macOS limitation!

