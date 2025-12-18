#!/bin/bash

# Create mount point if it doesn't exist
MOUNT_POINT="/tmp/fhir"
mkdir -p "$MOUNT_POINT"

# Build and run
cargo build --release
./target/release/fhir-fuse "$MOUNT_POINT" "http://localhost:8080/fhir"

