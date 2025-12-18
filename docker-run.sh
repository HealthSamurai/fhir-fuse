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

# Create mount directory if it doesn't exist
mkdir -p ./mnt

# Start docker-compose
docker-compose up "$@"

