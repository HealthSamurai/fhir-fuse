#!/bin/bash

# Cleanup script for stale FUSE mounts

echo "Cleaning up stale mounts..."

MOUNT_DIR="${MOUNT_DIR:-./mnt}"

# Stop any running containers
echo "Stopping containers..."
docker-compose down 2>/dev/null || true

# Try to unmount if mounted (macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Attempting to unmount (macOS)..."
    umount "$MOUNT_DIR" 2>/dev/null || true
    diskutil unmount force "$MOUNT_DIR" 2>/dev/null || true
fi

# Try to unmount if mounted (Linux)
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Attempting to unmount (Linux)..."
    fusermount -u "$MOUNT_DIR" 2>/dev/null || true
    umount "$MOUNT_DIR" 2>/dev/null || true
fi

# Remove and recreate the directory
echo "Recreating mount directory..."
rm -rf "$MOUNT_DIR" 2>/dev/null || true
mkdir -p "$MOUNT_DIR"

echo "✅ Cleanup complete!"
echo ""
echo "You can now run: ./docker-run.sh -d"

