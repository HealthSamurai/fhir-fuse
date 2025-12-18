#!/bin/bash

# Helper script to access FUSE-mounted files on macOS
# Since macOS Docker Desktop runs in a VM, FUSE mounts can't propagate to the host
# This script provides ways to access the files

CONTAINER_NAME="fhir-fuse-fhir-fuse-1"

show_help() {
    echo "FHIR-FUSE Access Helper (macOS)"
    echo ""
    echo "Usage:"
    echo "  ./access-fuse.sh ls [path]           - List files"
    echo "  ./access-fuse.sh cat <file>          - View file content"
    echo "  ./access-fuse.sh find <pattern>      - Find files"
    echo "  ./access-fuse.sh shell               - Open shell in container"
    echo "  ./access-fuse.sh copy <src> <dest>   - Copy file from container to host"
    echo "  ./access-fuse.sh sync                - Sync all files to ./mnt"
    echo ""
    echo "Examples:"
    echo "  ./access-fuse.sh ls Patient"
    echo "  ./access-fuse.sh cat Patient/patient-1.json"
    echo "  ./access-fuse.sh find '*.json'"
    echo "  ./access-fuse.sh copy Patient/patient-1.json ./patient-1.json"
    echo "  ./access-fuse.sh sync"
}

check_container() {
    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        echo "❌ Container $CONTAINER_NAME is not running"
        echo ""
        echo "Start it with: ./docker-run.sh -d"
        exit 1
    fi
}

case "${1:-help}" in
    ls)
        check_container
        path="${2:-.}"
        docker exec "$CONTAINER_NAME" ls -la "/mnt/fhir/$path"
        ;;
    
    cat)
        check_container
        if [ -z "$2" ]; then
            echo "❌ Please specify a file"
            echo "Usage: ./access-fuse.sh cat <file>"
            exit 1
        fi
        docker exec "$CONTAINER_NAME" cat "/mnt/fhir/$2"
        ;;
    
    find)
        check_container
        pattern="${2:-*}"
        docker exec "$CONTAINER_NAME" find /mnt/fhir -name "$pattern"
        ;;
    
    shell)
        check_container
        echo "Opening shell in container..."
        echo "FUSE mount is at: /mnt/fhir"
        echo "Exit with: exit or Ctrl+D"
        echo ""
        docker exec -it "$CONTAINER_NAME" sh
        ;;
    
    copy)
        check_container
        if [ -z "$2" ] || [ -z "$3" ]; then
            echo "❌ Please specify source and destination"
            echo "Usage: ./access-fuse.sh copy <src> <dest>"
            exit 1
        fi
        src="/mnt/fhir/$2"
        dest="$3"
        echo "Copying $src to $dest..."
        docker exec "$CONTAINER_NAME" cat "$src" > "$dest"
        echo "✅ Done"
        ;;
    
    sync)
        check_container
        echo "Syncing all files from container to ./mnt..."
        echo "This may take a while..."
        
        # Create directory structure
        mkdir -p ./mnt
        
        # Copy all files
        docker exec "$CONTAINER_NAME" tar -C /mnt/fhir -cf - . | tar -C ./mnt -xf -
        
        echo "✅ Files synced to ./mnt"
        ls -la ./mnt
        ;;
    
    help|--help|-h)
        show_help
        ;;
    
    *)
        echo "❌ Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac

