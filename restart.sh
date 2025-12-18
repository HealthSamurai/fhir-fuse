#!/bin/bash

# Script to rebuild and restart fhir-fuse

set -e

echo "=== Restarting FHIR-FUSE ==="
echo ""

# Kill existing process
echo "Stopping existing process..."
pkill -f "fhir-fuse.*./mnt" || echo "No existing process found"
sleep 1

# Unmount
echo "Unmounting..."
diskutil unmount force ./mnt 2>/dev/null || umount ./mnt 2>/dev/null || echo "Not mounted"
sleep 1

# Rebuild
echo ""
echo "Rebuilding..."
cargo build --release

# Check build time
echo ""
echo "Binary info:"
ls -lh ./target/release/fhir-fuse

echo ""
echo "Starting fhir-fuse..."
./target/release/fhir-fuse ./mnt http://localhost:8080/fhir &

# Wait for mount
sleep 3

# Check if mounted
echo ""
if mount | grep -q fhir-fuse; then
    echo "✅ Mounted successfully"
    mount | grep fhir-fuse
else
    echo "❌ Mount failed"
    exit 1
fi

echo ""
echo "✅ Ready! Try:"
echo "  cd mnt/Patient"
echo "  cp some-file.json test.json"

