# FHIR-FUSE Filesystem

FHIR-FUSE is a FUSE-based filesystem that exposes FHIR server data as a virtual filesystem.

## Features
- Mount FHIR server resources as files
- Browse patient data as JSON files
- Read-only access to FHIR resources
- Automatic refresh of patient data on mount

## Structure
- /Patient/ - Directory containing patient resources
  - Each patient is represented as a {patient_id}.json file

## Usage
Mount: fhir-fuse <mountpoint> <fhir_base_url>
Unmount: umount <mountpoint>

For more information, visit the project repository.