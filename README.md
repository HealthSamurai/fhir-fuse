# FHIR FUSE 

Представьте себе мир, в котором можно работать с FHIR данными как с обычными файлами, а сам FHIR сервер при этом - это просто папка на вашей файловой системе.

Вы сможете редактировать FHIR ресурсы вашим любимым текстовым редактором, копировать данный с сервера на сервер обычной командой "cp", и скриптоваться обычным bash?

Звучит как сказка? Она реальней чем вы себе представляете.
Благодаря технологии Filesystem in Userspace (FUSE) можно создать и замонтировать виртуальную файловую систему которая будет отображать данные вашего, самого лучшего, FHIR сервера.

## Quick Start with Docker

The easiest way to get started is using Docker Compose:

```sh
# Set your architecture (x86_64 or aarch64)
export TARGETARCH=x86_64

# Start all services (PostgreSQL, Aidbox, and FHIR-FUSE)
docker-compose up -d

# Access the mounted FHIR filesystem
ls ./mnt/Patient
```

The Docker setup includes:
- **PostgreSQL**: Database for Aidbox
- **Aidbox**: FHIR server
- **FHIR-FUSE**: Alpine-based container with FUSE filesystem mounted at `./mnt`

For more details, see [USAGE.md](USAGE.md).

## Dependencies

FUSE must be installed to build or run programs that use FUSE-Rust (i.e. kernel driver and libraries. Some platforms may also require userland utils like `fusermount`). A default installation of FUSE is usually sufficient.

To build FUSE-Rust or any program that depends on it, `pkg-config` needs to be installed as well.

### Linux

[FUSE for Linux] is available in most Linux distributions and usually called `fuse` or `fuse3` (this crate is compatible with both). To install on a Debian based system:

```sh
sudo apt-get install fuse3 libfuse3-dev
```

Install on CentOS:

```sh
sudo yum install fuse
```

To build, FUSE libraries and headers are required. The package is usually called `libfuse-dev` or `fuse-devel`. Also `pkg-config` is required for locating libraries and headers.

```sh
sudo apt-get install libfuse-dev pkg-config
```

```sh
sudo yum install fuse-devel pkgconfig
```

### macOS (untested)

Install [FUSE for macOS], which can be obtained from their website or installed using the Homebrew or Nix package managers. macOS version 10.9 or later is required. If you are using a Mac with Apple Silicon, you must also [enable support for third party kernel extensions][enable kext].


#### To install using Homebrew

```sh
brew install macfuse pkgconf
```

#### To install using Nix

``` sh
nix-env -iA nixos.macfuse-stubs nixos.pkg-config
```

When using `nix` it is required that you specify `PKG_CONFIG_PATH` environment variable to point at where `macfuse` is installed:

``` sh
export PKG_CONFIG_PATH=${HOME}/.nix-profile/lib/pkgconfig
```

### FreeBSD

Install packages `fusefs-libs` and `pkgconf`.

```sh
pkg install fusefs-libs pkgconf
```
