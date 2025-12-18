# FHIR-FUSE Filesystem

FHIR-FUSE is a FUSE-based filesystem that exposes FHIR server data as a virtual filesystem.

## Features
- **Dynamic Resource Discovery**: Automatically discovers available resources from the FHIR server's capability statement
- **Lazy Loading**: Resources are loaded on-demand when accessing directories for better performance
- **Universal Resource Support**: Supports all FHIR resource types exposed by the server (Patient, Observation, Encounter, etc.)
- **Read-only Access**: Safe, read-only access to FHIR resources
- **JSON Format**: All resources are presented as formatted JSON files for easy viewing

## Structure
The filesystem dynamically creates directories based on the server's capabilities:

```
/
├── README.md                          # This file
├── Patient/                           # Patient resources
│   ├── patient-001.json
│   ├── patient-002.json
│   └── ...
├── Observation/                       # Observation resources (if available)
│   ├── observation-001.json
│   └── ...
├── Encounter/                         # Encounter resources (if available)
│   └── ...
└── [Other Resource Types]/            # Any other resources supported by the server
```

## How It Works

1. **Capability Discovery**: On mount, FHIR-FUSE queries the server's `/metadata` endpoint to discover available resource types
2. **Directory Creation**: A directory is created for each supported resource type
3. **Lazy Loading**: Resources are fetched only when you access a directory for the first time
4. **Caching**: Once loaded, resources remain cached in memory until the filesystem is unmounted

## Usage

### Mount the filesystem
```bash
fhir-fuse <mountpoint> <fhir_base_url>

# Example:
fhir-fuse /tmp/fhir http://localhost:8080/fhir
```

### Browse resources
```bash
# List all available resource types
ls /tmp/fhir

# List all patients
ls /tmp/fhir/Patient

# View a patient record
cat /tmp/fhir/Patient/patient-123.json
```

### Unmount the filesystem
```bash
umount /tmp/fhir
```

## Offline Mode
Use `offline` as the FHIR base URL to mount the filesystem without connecting to a server:
```bash
fhir-fuse /tmp/fhir offline
```

## Requirements
- FHIR server with a standard REST API
- FUSE support on your operating system
- Network access to the FHIR server (unless in offline mode)

## Performance Notes
- Resources are loaded in batches (default: 100 per request)
- Only accessed resource types are loaded into memory
- The filesystem caches data to minimize server requests

For more information, visit the project repository.