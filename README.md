# 🔥 FHIR FUSE 🔥

**Work with FHIR data as if it were files on your filesystem.**

Imagine a world where your FHIR server is just a folder on your computer. You can:

- 📝 Edit FHIR resources with your favorite text editor
- 📋 Copy data between servers with `cp`
- 🔧 Script with standard bash commands
- 🔍 Search with `grep`, `find`, and `jq`

Sounds too good to be true? Thanks to **Filesystem in Userspace (FUSE)**, it's real. FHIR FUSE creates a virtual filesystem that mirrors your FHIR server's data, making healthcare interoperability as simple as working with files.

Project status: **proof of concept**

## 🚀 Quick Start with Docker

The easiest way to get started is using Docker Compose, which includes everything you need:

```bash
# Build binary for Linux ARM64
make build-linux-musl-arm64

# Start all services (PostgreSQL, Aidbox FHIR server, and FHIR-FUSE)
docker-compose up -d

# Access the mounted FHIR filesystem
# On Linux: ls ./mnt/Patient
# On macOS: see below for workarounds
docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir/Patient
```

> ⚠️ **Warning**
>  
> You need to initialize the Aidbox instance by nagivating to http://localhost:8080 and logging in.

**What's included:**

- 🐘 **PostgreSQL** - Database backend for Aidbox
- 🏥 **Aidbox** - Full-featured FHIR R4 server
- 📁 **FHIR-FUSE** - Alpine-based container with FUSE filesystem

### ⚠️ macOS Users

Due to Docker Desktop's VM architecture, FUSE mounts cannot propagate directly to the macOS host. However, there are simple workarounds:

#### Option 1: Native Execution (Recommended for Development)

```bash
# Install macFUSE
brew install macfuse

# Build and run natively
cargo build --release
./target/release/fhir-fuse /tmp/fhir http://localhost:8080/fhir

# Access files directly on your Mac
ls /tmp/fhir/Patient
cat /tmp/fhir/Patient/<id>.json | jq .
```

#### Option 2: Docker with Helper Script

```bash
# Start Docker services
docker-compose up -d

# Access files inside container (no scripts needed, just docker exec)
docker exec fhir-fuse-fhir-fuse-1 ls /mnt/fhir/Patient
docker exec fhir-fuse-fhir-fuse-1 cat /mnt/fhir/Patient/<id>.json | jq .

# Or copy files to host
docker cp fhir-fuse-fhir-fuse-1:/mnt/fhir ./mnt
```

See [MACOS_LIMITATIONS.md](MACOS_LIMITATIONS.md) for detailed explanations and additional workarounds.

For comprehensive usage instructions, see [USAGE.md](USAGE.md).

## 🔨 Building from Source

### Quick Build

```bash
# Build for your current platform
cargo build --release

# Cross-compile for all platforms (uses Docker with caching)
make build-all
```

**Performance Note:** Cross-compilation uses Docker volume caching. First build takes ~8 minutes, subsequent builds only ~45 seconds!

### Troubleshooting

If you encounter "transport endpoint is not connected" errors:

```bash
# Restart the FHIR-FUSE container
docker-compose restart fhir-fuse

# Or if running natively on macOS/Linux
umount /tmp/fhir  # or fusermount -u /tmp/fhir
./mount.sh
```

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for a complete troubleshooting guide.

## 📦 Dependencies

To build and run FHIR FUSE, you need FUSE libraries and `pkg-config`. A default FUSE installation is usually sufficient.

### Linux

FUSE is available in most Linux distributions as `fuse` or `fuse3` (both are compatible).

**Debian/Ubuntu:**

```bash
sudo apt-get install fuse3 libfuse3-dev pkg-config
```

**CentOS/RHEL:**

```bash
sudo yum install fuse-devel pkgconfig
```

### macOS

Install macFUSE using Homebrew:

```bash
brew install macfuse pkgconf
```

**Note:** macOS 10.9+ required. On Apple Silicon Macs, you may need to [enable third-party kernel extensions](https://developer.apple.com/documentation/security/disabling_and_enabling_system_integrity_protection).

#### Alternative: Using Nix

```bash
nix-env -iA nixos.macfuse-stubs nixos.pkg-config
export PKG_CONFIG_PATH=${HOME}/.nix-profile/lib/pkgconfig
```

### FreeBSD

```bash
pkg install fusefs-libs pkgconf
```

## 🏗️ Filesystem Design

### Basic Structure

Each FHIR resource type has its own directory, with individual resources as JSON files:

```text
./mnt/                              # Mount point
├── Patient/                        # Resource type directory
│   ├── patient-id-1.json          # Each file is a FHIR resource
│   └── patient-id-2.json          # Filename matches resource ID
├── Observation/
│   ├── observation-id-1.json
│   └── observation-id-2.json
├── Practitioner/
└── ...                             # All FHIR R4 resource types
```

### CRUD Operations

Work with FHIR resources using standard file operations:

- **Create**: `echo '{"resourceType":"Patient",...}' > ./mnt/Patient/new-patient.json`
- **Read**: `cat ./mnt/Patient/patient-id-1.json`
- **Update**: Edit the file with any text editor
- **Delete**: `rm ./mnt/Patient/patient-id-1.json`

### Resource History

Access historical versions of resources through hidden dot folders:

```text
./mnt/
├── Patient/
│   ├── patient-id-1.json              # Current version
│   ├── .patient-id-1/                 # Hidden history folder
│   │   ├── patient-id-1.v1.json      # Version 1
│   │   ├── patient-id-1.v2.json      # Version 2
│   │   └── patient-id-1.v3.json      # Version 3
│   └── patient-id-2.json
├── Observation/
│   ├── observation-id-1.json
│   └── .observation-id-1/
│       ├── observation-id-1.v1.json
│       └── observation-id-1.v2.json
```

**Usage:**

```bash
# View current version
cat ./mnt/Patient/patient-id-1.json

# View version history
ls ./mnt/Patient/.patient-id-1/

# Compare versions
diff ./mnt/Patient/.patient-id-1/patient-id-1.v1.json \
     ./mnt/Patient/.patient-id-1/patient-id-1.v2.json
```

### FHIR Search

Perform FHIR searches by creating directories with search parameters:

```text
./mnt/
├── Patient/
│   ├── _search/                                      # Search directory
│   │   ├── name=John/                               # Simple search
│   │   │   └── Patient/
│   │   │       ├── patient-1.json
│   │   │       └── patient-2.json
│   │   └── birthdate=gt1990-01-01&gender=male/     # Complex search
│   │       └── Patient/
│   │           └── patient-3.json
│   ├── patient-1.json
│   └── patient-2.json
├── Observation/
│   ├── _search/
│   │   └── _include=Observation:patient&_include:iterate=Patient:link/
│   │       ├── Observation/                         # Search results include
│   │       │   ├── observation-1.json              # related resources
│   │       │   └── observation-2.json
│   │       └── Patient/                             # via _include
│   │           ├── patient-1.json
│   │           └── patient-2.json
│   ├── observation-1.json
│   └── observation-2.json
```

**Usage:**

```bash
# Create a search directory (mkdir triggers the search)
mkdir -p "./mnt/Patient/_search/name=Smith"

# View search results
ls "./mnt/Patient/_search/name=Smith/Patient/"

# Complex searches with multiple parameters
mkdir -p "./mnt/Observation/_search/code=http://loinc.org|85354-9&date=gt2023-01-01"
ls "./mnt/Observation/_search/code=http://loinc.org|85354-9&date=gt2023-01-01/Observation/"
```

### FHIR Operations

Execute FHIR operations through special `$operation` directories:

```text
./mnt/
├── Patient/
│   └── $validate/                          # Operation directory
│       └── resource-id/                    # Operation parameters
│           └── result.json                 # Operation result
└── ViewDefinition/
    ├── $run/
    │   ├── view-id.json                   # Touch to execute, read for results
    │   └── view-id.csv                    # Different output formats
    ├── patient_demographics.json          # ViewDefinition resources
    └── blood_pressure.json
```

#### Example: Running a ViewDefinition

```bash
# List available ViewDefinitions
ls ./mnt/ViewDefinition/

# Execute a ViewDefinition (touch creates the operation)
touch "./mnt/ViewDefinition/\$run/patient_demographics.json"

# Read the results
cat "./mnt/ViewDefinition/\$run/patient_demographics.json" | jq .

# Get results in CSV format
touch "./mnt/ViewDefinition/\$run/patient_demographics.csv"
cat "./mnt/ViewDefinition/\$run/patient_demographics.csv"
```

**Equivalent REST API:**

```http
POST /fhir/ViewDefinition/patient_demographics/$run
Content-Type: application/json
Accept: application/json

{
  "resourceType": "Parameters",
  "parameter": [{
    "name": "_format",
    "valueCode": "json"
  }]
}
```

## 💡 Use Cases

### Data Migration

```bash
# Copy all patients from one server to another
cp -r /mnt/source-server/Patient/* /mnt/destination-server/Patient/
```

### Backup & Export

```bash
# Backup all observations to a tar archive
tar -czf observations-backup.tar.gz /mnt/fhir/Observation/

# Export specific resources
cp /mnt/fhir/Patient/patient-123.json ./backups/
```

### Data Analysis

```bash
# Count resources by type
find /mnt/fhir -name "*.json" | wc -l

# Extract specific fields with jq
cat /mnt/fhir/Patient/*.json | jq '.name[0].family'

# Search for patterns
grep -r "diabetes" /mnt/fhir/Condition/
```

### Scripting & Automation

```bash
# Batch update resources
for file in /mnt/fhir/Patient/*.json; do
  jq '.active = true' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
done

# Monitor changes
watch -n 5 'ls -l /mnt/fhir/Patient/'
```

### Development & Testing

```bash
# Quickly inspect test data
cat /mnt/fhir/Patient/test-patient-1.json | jq .

# Create test fixtures
cp /mnt/fhir/Patient/example.json ./test/fixtures/

# Validate resources
for file in /mnt/fhir/Patient/*.json; do
  jq empty "$file" || echo "Invalid JSON: $file"
done
```

## ✨ Features

- 🔄 **Full CRUD Support** - Create, read, update, and delete resources as files
- 📚 **Version History** - Access historical versions through hidden folders
- 🔍 **FHIR Search** - Execute searches by creating directories
- ⚡ **FHIR Operations** - Run `$validate`, `$run`, and other operations
- 🔗 **Include Support** - Search results include related resources via `_include`
- 🏥 **FHIR R4 Compliant** - Works with any FHIR R4 server
- 🐳 **Docker Ready** - Includes complete Docker Compose setup
- 🍎 **Cross-Platform** - Linux, macOS, and FreeBSD support
- 🚀 **High Performance** - Efficient caching and lazy loading
- 🛠️ **Standard Tools** - Use `ls`, `cat`, `grep`, `jq`, and more

## 📚 Documentation

- [QUICKSTART.md](QUICKSTART.md) - Get started in 5 minutes
- [USAGE.md](USAGE.md) - Comprehensive usage guide
- [IMPLEMENTATION.md](IMPLEMENTATION.md) - Technical implementation details
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Common issues and solutions
- [MACOS_LIMITATIONS.md](MACOS_LIMITATIONS.md) - macOS-specific workarounds
- [DOCKER.md](DOCKER.md) - Docker deployment guide
- [MOUNT_PROPAGATION.md](MOUNT_PROPAGATION.md) - How mount propagation works

## 🤝 Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## 📄 License

This project is open source. See the LICENSE file for details.

## 🙏 Acknowledgments

Built with:

- [fuse-rust](https://github.com/cberner/fuser) - Rust bindings for FUSE
- [Aidbox](https://www.health-samurai.io/aidbox) - FHIR server platform
- [FHIR R4](https://hl7.org/fhir/R4/) - Fast Healthcare Interoperability Resources

---

**Made with ❤️ for the healthcare interoperability community**
