use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};

mod text_file;
use text_file::TextFile;

mod directory;
use directory::{Directory, DirectoryListing};

mod capability;
use capability::{fetch_capability_statement, fetch_resources, ServerCapabilities};

mod inode_allocator;
use inode_allocator::InodeAllocator;

const TTL: Duration = Duration::from_secs(1);

const README_CONTENT: &str = include_str!("../assets/README.md");

#[derive(Debug, Clone)]
struct ResourceFile {
    _inode: u64,
    _name: String,
    content: String,
    resource_type: String,
}

struct FhirFuse {
    fhir_base_url: String,
    resources: HashMap<u64, ResourceFile>,
    resource_name_to_inode: HashMap<String, HashMap<String, u64>>, // resource_type -> (filename -> inode)
    text_files: HashMap<u64, TextFile>,
    directories: HashMap<u64, Directory>,
    resource_directories: HashMap<String, u64>, // resource_type -> directory_inode
    loaded_resources: HashSet<String>,          // Track which resource types have been loaded
    _capabilities: Option<ServerCapabilities>,
    inode_allocator: InodeAllocator,
    root_inode: u64,
    _patient_dir_inode: u64,
    _readme_inode: u64,
}

impl FhirFuse {
    fn new(fhir_base_url: String) -> Self {
        let mut inode_allocator = InodeAllocator::new(1);

        let root_inode = inode_allocator.allocate();
        let patient_dir_inode = inode_allocator.allocate();
        let readme_inode = inode_allocator.allocate();

        let mut text_files = HashMap::new();
        let readme = TextFile::new(readme_inode, "README.md", README_CONTENT);
        text_files.insert(readme_inode, readme);

        let mut directories = HashMap::new();
        directories.insert(root_inode, Directory::new(root_inode, "/"));
        directories.insert(
            patient_dir_inode,
            Directory::new(patient_dir_inode, "Patient"),
        );

        let mut resource_directories = HashMap::new();
        resource_directories.insert("Patient".to_string(), patient_dir_inode);

        let mut capabilities = None;

        // Fetch capabilities and create directories for each resource type
        if fhir_base_url != "offline" {
            match fetch_capability_statement(&fhir_base_url) {
                Ok(caps) => {
                    println!("Successfully fetched capabilities");
                    for resource_type in &caps.resources {
                        if resource_type != "Patient" {
                            // Patient already created
                            let dir_inode = inode_allocator.allocate();
                            directories.insert(
                                dir_inode,
                                Directory::new(dir_inode, resource_type.clone()),
                            );
                            resource_directories.insert(resource_type.clone(), dir_inode);
                        }
                    }
                    capabilities = Some(caps);
                }
                Err(e) => {
                    eprintln!("Failed to fetch capabilities: {:#?}", e);
                }
            }
        }

        let fs = FhirFuse {
            fhir_base_url: fhir_base_url.clone(),
            resources: HashMap::new(),
            resource_name_to_inode: HashMap::new(),
            text_files,
            directories,
            resource_directories,
            loaded_resources: HashSet::new(),
            _capabilities: capabilities,
            inode_allocator,
            root_inode,
            _patient_dir_inode: patient_dir_inode,
            _readme_inode: readme_inode,
        };
        // Don't load resources immediately - use lazy loading
        fs
    }

    fn ensure_resources_loaded(&mut self, resource_type: &str) {
        if self.fhir_base_url == "offline" {
            return;
        }

        if !self.loaded_resources.contains(resource_type) {
            self.refresh_resources(resource_type);
            self.loaded_resources.insert(resource_type.to_string());
        }
    }

    fn refresh_resources(&mut self, resource_type: &str) {
        println!("Fetching {} resources from FHIR server...", resource_type);

        match fetch_resources(&self.fhir_base_url, resource_type, Some(100)) {
            Ok(resources) => {
                // Clear old resources of this type
                self.resources
                    .retain(|_, r| r.resource_type != resource_type);
                self.resource_name_to_inode.remove(resource_type);

                let mut name_to_inode = HashMap::new();

                for resource in resources {
                    let inode = self.inode_allocator.allocate();
                    let id = resource["id"].as_str().unwrap_or("unknown");
                    let filename = format!("{}.json", id);
                    let content = serde_json::to_string_pretty(&resource).unwrap_or_default();

                    let resource_file = ResourceFile {
                        _inode: inode,
                        _name: filename.clone(),
                        content,
                        resource_type: resource_type.to_string(),
                    };

                    self.resources.insert(inode, resource_file);
                    name_to_inode.insert(filename, inode);
                }

                println!("Loaded {} {} resources", name_to_inode.len(), resource_type);
                self.resource_name_to_inode
                    .insert(resource_type.to_string(), name_to_inode);
            }
            Err(e) => {
                eprintln!("Failed to fetch {} resources: {:#?}", resource_type, e);
            }
        }
    }

    fn get_attrs(&self, inode: u64) -> Option<FileAttr> {
        if let Some(directory) = self.directories.get(&inode) {
            return Some(directory.get_attr());
        }

        let ts = SystemTime::now();

        match inode {
            _ => {
                if let Some(resource) = self.resources.get(&inode) {
                    Some(FileAttr {
                        ino: inode,
                        size: resource.content.len() as u64,
                        blocks: 1,
                        atime: ts,
                        mtime: ts,
                        ctime: ts,
                        crtime: ts,
                        kind: FileType::RegularFile,
                        perm: 0o644,
                        nlink: 1,
                        uid: 501,
                        gid: 20,
                        rdev: 0,
                        flags: 0,
                        blksize: 512,
                    })
                } else {
                    None
                }
            }
        }
    }
}

impl Filesystem for FhirFuse {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = name.to_str().unwrap_or("");

        match parent {
            parent if parent == self.root_inode => {
                // Check if it's a resource directory
                if let Some(&dir_inode) = self.resource_directories.get(name_str) {
                    if let Some(attr) = self.get_attrs(dir_inode) {
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
                } else if let Some(text_file) =
                    self.text_files.values().find(|f| f.name == name_str)
                {
                    let attr = text_file.get_attr();
                    reply.entry(&TTL, &attr, 0);
                    return;
                }
            }
            parent => {
                // Check if parent is a resource directory
                let matching_resource = self
                    .resource_directories
                    .iter()
                    .find(|(_, &dir_inode)| parent == dir_inode)
                    .map(|(resource_type, _)| resource_type.clone());

                if let Some(resource_type) = matching_resource {
                    // Load resources on first access
                    self.ensure_resources_loaded(&resource_type);
                    if let Some(name_map) = self.resource_name_to_inode.get(&resource_type) {
                        if let Some(&inode) = name_map.get(name_str) {
                            if let Some(attr) = self.get_attrs(inode) {
                                reply.entry(&TTL, &attr, 0);
                                return;
                            }
                        }
                    }
                }
            }
        }

        reply.error(ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        if let Some(directory) = self.directories.get(&ino) {
            let attr = directory.get_attr();
            reply.attr(&TTL, &attr);
        } else if let Some(text_file) = self.text_files.get(&ino) {
            let attr = text_file.get_attr();
            reply.attr(&TTL, &attr);
        } else if let Some(attr) = self.get_attrs(ino) {
            reply.attr(&TTL, &attr);
        } else {
            reply.error(ENOENT);
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        if let Some(text_file) = self.text_files.get(&ino) {
            let data = text_file.read(offset, size);
            reply.data(&data);
        } else if let Some(resource) = self.resources.get(&ino) {
            let content = resource.content.as_bytes();
            let offset = offset as usize;
            let size = size as usize;

            if offset < content.len() {
                let end = std::cmp::min(offset + size, content.len());
                reply.data(&content[offset..end]);
            } else {
                reply.data(&[]);
            }
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        match ino {
            ino if ino == self.root_inode => {
                let mut listing = DirectoryListing::new();
                listing.add_current_dir(self.root_inode);
                listing.add_parent_dir(self.root_inode);

                // Add resource directories
                let mut resource_dirs: Vec<_> = self.resource_directories.iter().collect();
                resource_dirs.sort_by_key(|(name, _)| name.as_str());
                for (name, &inode) in resource_dirs {
                    listing.add_dir(inode, name);
                }

                for text_file in self.text_files.values() {
                    listing.add_file(text_file.inode, &text_file.name);
                }

                let entries = listing.into_vec();
                for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                    if reply.add(entry.0, (i + 1) as i64, entry.1, &entry.2) {
                        break;
                    }
                }
                reply.ok();
            }
            ino => {
                // Check if this is a resource directory
                let matching_resource = self
                    .resource_directories
                    .iter()
                    .find(|(_, &dir_inode)| ino == dir_inode)
                    .map(|(resource_type, &dir_inode)| (resource_type.clone(), dir_inode));

                if let Some((resource_type, dir_inode)) = matching_resource {
                    // Load resources on first access
                    self.ensure_resources_loaded(&resource_type);

                    let mut listing = DirectoryListing::new();
                    listing.add_current_dir(dir_inode);
                    listing.add_parent_dir(self.root_inode);

                    // Add resource files for this type
                    if let Some(name_map) = self.resource_name_to_inode.get(&resource_type) {
                        let mut files: Vec<_> = name_map.iter().collect();
                        files.sort_by_key(|(name, _)| name.as_str());
                        for (name, &inode) in files {
                            listing.add_file(inode, name);
                        }
                    }

                    let entries = listing.into_vec();
                    for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                        if reply.add(entry.0, (i + 1) as i64, entry.1, &entry.2) {
                            break;
                        }
                    }
                    reply.ok();
                } else {
                    reply.error(ENOENT);
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <mountpoint> <fhir_base_url>", args[0]);
        eprintln!("Example: {} /tmp/fhir http://localhost:8080/fhir", args[0]);
        std::process::exit(1);
    }

    let mountpoint = &args[1];
    let fhir_base_url = &args[2];

    println!("Mounting FHIR filesystem at: {}", mountpoint);
    println!("FHIR server: {}", fhir_base_url);

    let fs = FhirFuse::new(fhir_base_url.clone());

    let options = vec![
        MountOption::RO,
        MountOption::FSName("fhir-fuse".to_string()),
    ];

    match fuser::mount2(fs, mountpoint, &options) {
        Ok(_) => println!("Filesystem unmounted"),
        Err(e) => eprintln!("Failed to mount filesystem: {}", e),
    }
}
