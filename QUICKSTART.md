# Quick Start Guide

## Prerequisites

Make sure you have:
1. **macFUSE** installed: `brew install macfuse`
2. **FHIR server** running at `http://localhost:8080/fhir`

## Quick Start

```bash
# 1. Build the project
cargo build --release

# 2. Create mount point
mkdir -p /tmp/fhir

# 3. Mount the filesystem
./target/release/fhir-fuse /tmp/fhir http://localhost:8080/fhir
```

In another terminal:

```bash
# List all patients
ls /tmp/fhir/Patient

# View a patient
cat /tmp/fhir/Patient/<patient-id>.json

# Pretty print with jq
cat /tmp/fhir/Patient/<patient-id>.json | jq .
```

To unmount (in another terminal):

```bash
# macOS
umount /tmp/fhir

# Or just press Ctrl+C in the terminal running the filesystem
```

## Using Helper Scripts

```bash
# Mount (builds and mounts automatically)
./mount.sh

# Unmount (in another terminal)
./unmount.sh
```

## Example Commands

```bash
# Count patients
ls /tmp/fhir/Patient | wc -l

# Search for a name
grep -r "John" /tmp/fhir/Patient/

# List patient IDs
ls /tmp/fhir/Patient | sed 's/.json$//'

# View multiple patients
for file in /tmp/fhir/Patient/*.json; do
  echo "=== $file ==="
  cat "$file" | jq '.name[0]'
done
```

## Troubleshooting

**Problem**: "No such file or directory" when accessing `/tmp/fhir`
**Solution**: Make sure the filesystem is mounted and running

**Problem**: "Transport endpoint is not connected"
**Solution**: The filesystem crashed. Unmount and remount:
```bash
umount /tmp/fhir  # or fusermount -u /tmp/fhir on Linux
./mount.sh
```

**Problem**: No patients showing up
**Solution**: Check that your FHIR server has Patient resources:
```bash
curl http://localhost:8080/fhir/Patient
```

## What's Next?

See `IMPLEMENTATION.md` for technical details and `USAGE.md` for comprehensive usage guide.

