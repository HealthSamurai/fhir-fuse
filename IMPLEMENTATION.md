# FHIR FUSE Implementation Details

## What Was Built

A FUSE (Filesystem in Userspace) implementation in Rust that exposes FHIR Patient resources from a FHIR server as files in a virtual filesystem.

## Architecture

### Components

1. **FhirFuse Struct**: Main filesystem implementation
   - Manages connection to FHIR server
   - Caches patient data in memory
   - Implements FUSE filesystem operations

2. **Data Structures**:
   - `FhirBundle`: Deserializes FHIR Bundle responses
   - `PatientFile`: Represents a patient as a virtual file
   - Inode mapping for filesystem navigation

3. **FUSE Operations Implemented**:
   - `lookup`: Find files/directories by name
   - `getattr`: Get file/directory attributes
   - `read`: Read file contents
   - `readdir`: List directory contents

## Directory Structure

```
/ (root)
└── Patient/
    ├── patient-1.json
    ├── patient-2.json
    └── patient-3.json
```

## How It Works

### 1. Initialization

When the filesystem starts:
- Connects to the FHIR server
- Fetches all Patient resources via `GET /Patient`
- Parses the FHIR Bundle response
- Creates virtual files for each patient
- Assigns unique inodes to each file

### 2. File System Operations

**Listing directories** (`ls /tmp/fhir`):
- Returns the "Patient" directory

**Listing patients** (`ls /tmp/fhir/Patient`):
- Returns all patient files (e.g., `patient-1.json`)

**Reading a patient** (`cat /tmp/fhir/Patient/patient-1.json`):
- Returns the cached JSON content for that patient

### 3. Inode Management

- Root directory: inode 1
- Patient directory: inode 2
- Patient files: inodes 100+

## Key Features

### ✅ Implemented

- Read-only filesystem
- Fetches all patients from FHIR server
- Pretty-printed JSON output
- Standard POSIX file operations
- Error handling for missing resources

### 🚧 Not Yet Implemented (Future Enhancements)

- Write operations (create/update/delete patients)
- Other resource types (Observation, Encounter, etc.)
- Pagination for large patient lists
- Real-time updates/refresh
- Caching with TTL
- Search parameters as subdirectories

## Code Flow

```
main()
  ├─> Parse command line arguments
  ├─> Create FhirFuse instance
  │   └─> refresh_patients()
  │       ├─> fetch_patients() - HTTP GET to FHIR server
  │       └─> Parse and cache patient data
  └─> Mount filesystem with fuser::mount2()

User Operations:
  ls /tmp/fhir
    └─> readdir(ROOT_INODE) -> returns [".", "..", "Patient"]
  
  ls /tmp/fhir/Patient
    └─> lookup("Patient") -> returns PATIENT_DIR_INODE
    └─> readdir(PATIENT_DIR_INODE) -> returns patient files
  
  cat /tmp/fhir/Patient/patient-1.json
    └─> lookup("patient-1.json") -> returns file inode
    └─> getattr(inode) -> returns file attributes
    └─> read(inode, offset, size) -> returns JSON content
```

## Dependencies

- **fuser**: FUSE bindings for Rust
- **reqwest**: HTTP client for FHIR API calls
- **serde/serde_json**: JSON serialization/deserialization
- **anyhow**: Error handling
- **libc**: POSIX error codes

## Testing

To test the implementation:

1. Start your FHIR server
2. Mount the filesystem: `./mount.sh`
3. In another terminal, browse the files:
   ```bash
   ls /tmp/fhir/Patient
   cat /tmp/fhir/Patient/<patient-id>.json
   ```

## Performance Considerations

- All patients are loaded into memory at startup
- No pagination (could be an issue with thousands of patients)
- No caching refresh mechanism
- Blocking HTTP calls (could add async support)

## Security Considerations

- Read-only filesystem (safe for browsing)
- No authentication implemented (relies on network security)
- No input validation on FHIR responses (trusts server)

## Next Steps

To extend this implementation:

1. **Add more resource types**: Create directories for Observation, Encounter, etc.
2. **Implement write operations**: Allow creating/updating resources
3. **Add search support**: Create subdirectories for search parameters
4. **Implement pagination**: Handle large datasets efficiently
5. **Add caching**: Implement TTL-based cache refresh
6. **Add authentication**: Support OAuth2/Bearer tokens

