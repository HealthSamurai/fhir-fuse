# FHIR-FUSE Filesystem

FHIR-FUSE is a FUSE-based filesystem that exposes FHIR server data as a virtual filesystem.

## Features
- **Dynamic Resource Discovery**: Automatically discovers available resources from the FHIR server's capability statement
- **Lazy Loading**: Resources are loaded on-demand when accessing directories for better performance
- **Universal Resource Support**: Supports all FHIR resource types exposed by the server (Patient, Observation, Encounter, etc.)
- **CRUD Operations**: Create, read, update, and delete FHIR resources directly through the filesystem
- **Resource History**: Access historical versions of resources through hidden dot directories
- **JSON Format**: All resources are presented as formatted JSON files for easy viewing

## Structure
The filesystem dynamically creates directories based on the server's capabilities:

```
/
├── README.md                          # This file
├── Patient/                           # Patient resources
│   ├── patient-001.json
│   ├── .patient-001/                 # Hidden directory with resource history
│   │   ├── patient-001.v1.json       # Version 1
│   │   ├── patient-001.v2.json       # Version 2
│   │   └── patient-001.v3.json       # Version 3
│   ├── patient-002.json
│   ├── .patient-002/                 # Hidden directory with resource history
│   │   └── patient-002.v1.json       # Version 1
│   └── ...
├── Observation/                       # Observation resources (if available)
│   ├── observation-001.json
│   ├── .observation-001/              # Hidden directory with resource history
│   │   ├── observation-001.v1.json   # Version 1
│   │   └── observation-001.v2.json   # Version 2
│   └── ...
├── Encounter/                         # Encounter resources (if available)
│   └── ...
└── [Other Resource Types]/            # Any other resources supported by the server
```

## How It Works

1. **Capability Discovery**: On mount, FHIR-FUSE queries the server's `/metadata` endpoint to discover available resource types
2. **Directory Creation**: A directory is created for each supported resource type
3. **Lazy Loading**: Resources are fetched only when you access a directory for the first time
4. **History Directories**: Hidden directories (`.resource-id/`) are created automatically for each resource to store version history
5. **On-demand History**: Resource history is fetched from the server only when accessing the history directory
6. **Caching**: Resources and history are cached with a 5-second expiry to balance freshness and performance

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

# View resource history
ls -la /tmp/fhir/Patient/.patient-123/
cat /tmp/fhir/Patient/.patient-123/patient-123.v1.json

# Create a new resource
echo '{"resourceType": "Patient", "name": [{"family": "Doe", "given": ["John"]}]}' > /tmp/fhir/Patient/new-patient.json

# Update a resource
vi /tmp/fhir/Patient/patient-123.json

# Delete a resource
rm /tmp/fhir/Patient/patient-123.json
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
- Resource history is loaded on-demand when history directories are accessed
- Cache expires after 5 seconds to ensure data freshness

## CRUD Operations

### Create
Create a new resource by creating a JSON file in the appropriate resource directory:
```bash
echo '{"resourceType": "Patient", ...}' > /tmp/fhir/Patient/new-patient.json
```

### Read
Read resources using standard file operations:
```bash
cat /tmp/fhir/Patient/patient-123.json
```

### Update
Edit resources using any text editor:
```bash
vi /tmp/fhir/Patient/patient-123.json
```

### Delete
Remove resources using standard file deletion:
```bash
rm /tmp/fhir/Patient/patient-123.json
```

## Resource History
Each resource has a hidden directory containing its version history:
- History directories are named `.{resource-id}/`
- Version files are named `{resource-id}.v{version}.json`
- History is fetched on-demand when the directory is accessed
- All versions are read-only

For more information, visit the project repository.
