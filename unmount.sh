#!/bin/bash

# Unmount the filesystem
MOUNT_POINT="/tmp/fhir"

if [[ "$OSTYPE" == "darwin"* ]]; then
    umount "$MOUNT_POINT"
else
    fusermount -u "$MOUNT_POINT"
fi

echo "Unmounted $MOUNT_POINT"

