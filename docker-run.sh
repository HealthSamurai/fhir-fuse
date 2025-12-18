#!/bin/bash

# Detect architecture and set TARGETARCH
ARCH=$(uname -m)

if [ "$ARCH" = "x86_64" ]; then
    export TARGETARCH="x86_64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    export TARGETARCH="aarch64"
else
    echo "Unsupported architecture: $ARCH"
    echo "Please set TARGETARCH manually to either x86_64 or aarch64"
    exit 1
fi

echo "Detected architecture: $ARCH"
echo "Using TARGETARCH: $TARGETARCH"

# Set mount directory (can be overridden with MOUNT_DIR env var)
MOUNT_DIR="${MOUNT_DIR:-./mnt}"
export MOUNT_DIR

# Check if mount directory has stale mount
if ! ls "$MOUNT_DIR" >/dev/null 2>&1; then
    echo "⚠️  Detected stale mount at $MOUNT_DIR"
    echo "Cleaning up..."
    
    # Try to unmount (macOS)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        umount "$MOUNT_DIR" 2>/dev/null || true
        diskutil unmount force "$MOUNT_DIR" 2>/dev/null || true
    fi
    
    # Try to unmount (Linux)
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        fusermount -u "$MOUNT_DIR" 2>/dev/null || true
        umount "$MOUNT_DIR" 2>/dev/null || true
    fi
    
    # Recreate directory
    rm -rf "$MOUNT_DIR" 2>/dev/null || true
    mkdir -p "$MOUNT_DIR"
    echo "✅ Cleanup complete"
else
    # Create mount directory if it doesn't exist
    mkdir -p "$MOUNT_DIR"
fi

# Make sure the mount point is shared on the host (Linux only)
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "Setting up mount propagation for $MOUNT_DIR..."
    # Make the mount point shared so FUSE mounts inside container are visible on host
    sudo mount --make-rshared "$MOUNT_DIR" 2>/dev/null || true
fi

echo "Mount directory: $MOUNT_DIR"
echo ""

# Start docker-compose
docker-compose up "$@"

