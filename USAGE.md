# FHIR FUSE Usage Guide

## Overview

This FUSE filesystem allows you to browse FHIR Patient resources as files in a virtual filesystem. Each patient appears as a JSON file in the `Patient` directory.

## Prerequisites

1. **macOS**: Install macFUSE
   ```bash
   brew install macfuse
   ```

2. **FHIR Server**: Make sure your Aidbox FHIR server is running at `http://localhost:8080/fhir`

## Building

```bash
cargo build --release
```

## Running

### Option 1: Using Docker Compose (Recommended)

The easiest way to run FHIR FUSE is with Docker Compose, which includes Aidbox FHIR server and PostgreSQL:

```bash
# Set the target architecture (x86_64 or aarch64)
export TARGETARCH=x86_64  # or aarch64 for ARM64

# Start all services
docker-compose up -d

# The FHIR filesystem will be mounted at ./mnt directory
ls ./mnt/Patient
```

To stop the services:

```bash
docker-compose down
```

**Note**: The fhir-fuse container requires privileged mode and access to `/dev/fuse` to mount the filesystem. The mounted filesystem will be available in the `./mnt` directory on your host machine thanks to `rshared` bind propagation, which makes the FUSE mount inside the container visible on the host.

For detailed information about how mount propagation works, see [MOUNT_PROPAGATION.md](MOUNT_PROPAGATION.md).

### Option 2: Using the mount script

```bash
./mount.sh
```

This will:
- Create a mount point at `/tmp/fhir`
- Build the project
- Mount the FHIR filesystem

### Option 2: Manual mounting

```bash
# Create mount point
mkdir -p /tmp/fhir

# Run the filesystem
./target/release/fhir-fuse /tmp/fhir http://localhost:8080/fhir
```

## Using the Filesystem

Once mounted, you can browse the filesystem:

```bash
# List the root directory (shows Patient folder)
ls /tmp/fhir

# List all patients
ls /tmp/fhir/Patient

# View a specific patient
cat /tmp/fhir/Patient/<patient-id>.json

# Pretty print a patient
jq . /tmp/fhir/Patient/<patient-id>.json

# Count patients
ls /tmp/fhir/Patient | wc -l

# Search for patients with specific data
grep -r "John" /tmp/fhir/Patient/
```

## Unmounting

### Option 1: Using the unmount script

```bash
./unmount.sh
```

### Option 2: Manual unmounting

On macOS:
```bash
umount /tmp/fhir
```

On Linux:
```bash
fusermount -u /tmp/fhir
```

### Option 3: Stop the process

Press `Ctrl+C` in the terminal where the filesystem is running.

## Example Session

```bash
# Terminal 1: Mount the filesystem
$ ./mount.sh
Mounting FHIR filesystem at: /tmp/fhir
FHIR server: http://localhost:8080/fhir
Fetching patients from FHIR server...
Loaded 5 patients

# Terminal 2: Browse the filesystem
$ ls /tmp/fhir
Patient

$ ls /tmp/fhir/Patient
patient-1.json  patient-2.json  patient-3.json

$ cat /tmp/fhir/Patient/patient-1.json
{
  "resourceType": "Patient",
  "id": "patient-1",
  "name": [
    {
      "family": "Doe",
      "given": ["John"]
    }
  ],
  ...
}
```

## Features

- **Read-only filesystem**: All patient data is read-only
- **Automatic refresh**: Patient data is fetched when the filesystem starts
- **JSON format**: Each patient is stored as a pretty-printed JSON file
- **Standard tools**: Use any standard Unix tools (ls, cat, grep, etc.)

## Troubleshooting

### "Transport endpoint is not connected"

The filesystem has crashed or been unmounted. Unmount and remount:

```bash
./unmount.sh
./mount.sh
```

### "Permission denied" when mounting

Make sure you have macFUSE installed and that kernel extensions are enabled.

### "Cannot fetch patients"

Check that:
1. Your FHIR server is running
2. The URL is correct (default: `http://localhost:8080/fhir`)
3. You have network connectivity

### No patients showing up

The FHIR server might not have any Patient resources. Create some test patients first.

