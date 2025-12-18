use fuser::{
    FileAttr, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::ENOENT;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::time::Duration;

mod vfs;
use vfs::{Directory, DirectoryListing, FHIRResource, IndexStats, InodeIndex, TextFile, VFSEntry};

mod capability;
use capability::{fetch_capability_statement, fetch_resources};

mod inode_allocator;
use inode_allocator::InodeAllocator;

const TTL: Duration = Duration::from_secs(1);

const README_CONTENT: &str = include_str!("../assets/README.md");

struct FhirFuse {
    fhir_base_url: String,
    inode_index: InodeIndex,
    resource_directories: HashMap<String, u64>, // resource_type -> directory_inode
    loaded_resources: HashSet<String>,          // Track which resource types have been loaded
    inode_allocator: InodeAllocator,
}

impl FhirFuse {
    fn new(fhir_base_url: String) -> Self {
        let mut inode_allocator = InodeAllocator::new(1);

        let root_inode = inode_allocator.root_inode;
        let readme_inode = inode_allocator.allocate();

        let mut inode_index = InodeIndex::new();

        // Add root directory
        inode_index.insert_directory(Directory::new(root_inode, "/"));

        // Add README file
        let readme = TextFile::new(readme_inode, "README.md", README_CONTENT);
        inode_index.insert_text_file(readme);
        inode_index.add_parent_child_relation(root_inode, readme_inode);

        let mut resource_directories = HashMap::new();

        // Fetch capabilities and create directories for each resource type
        match fetch_capability_statement(&fhir_base_url) {
            Ok(caps) => {
                println!("Successfully fetched capabilities");
                for resource_type in &caps.resources {
                    let dir_inode = inode_allocator.allocate();
                    println!("inode {} <- {}", dir_inode, resource_type);
                    inode_index.insert_directory(Directory::new(dir_inode, resource_type.clone()));
                    inode_index.add_parent_child_relation(root_inode, dir_inode);
                    resource_directories.insert(resource_type.clone(), dir_inode);
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch capabilities: {:#?}", e);
            }
        }

        let fs = FhirFuse {
            fhir_base_url: fhir_base_url.clone(),
            inode_index,
            resource_directories,
            loaded_resources: HashSet::new(),
            inode_allocator,
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
                self.inode_index.clear_resources_by_type(resource_type);

                // Get the directory inode for this resource type
                let dir_inode = self.resource_directories.get(resource_type).copied();

                let mut count = 0;
                for resource in resources {
                    let inode = self.inode_allocator.allocate();
                    let id = resource["id"].as_str().unwrap_or("unknown");
                    let content = serde_json::to_string_pretty(&resource).unwrap_or_default();

                    let resource_entry = FHIRResource::new(inode, resource_type, id, content);

                    self.inode_index.insert_resource(resource_entry);

                    // Add parent-child relation if we have the directory
                    if let Some(dir) = dir_inode {
                        self.inode_index.add_parent_child_relation(dir, inode);
                    }
                    count += 1;
                }

                println!("Loaded {} {} resources", count, resource_type);
            }
            Err(e) => {
                eprintln!("Failed to fetch {} resources: {:#?}", resource_type, e);
            }
        }
    }

    fn get_attrs(&self, inode: u64) -> Option<FileAttr> {
        self.inode_index.get_attr(inode)
    }

    #[allow(dead_code)]
    fn debug_print_stats(&self) {
        let stats: IndexStats = self.inode_index.stats();
        println!("=== Inode Index Statistics ===");
        println!("{}", stats);
        println!("Loaded resource types: {:?}", self.loaded_resources);
        println!(
            "Resource directories: {} types",
            self.resource_directories.len()
        );
        println!("Next inode: {}", self.inode_allocator.peek_next());
        println!("==============================");
    }
}

impl Filesystem for FhirFuse {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = name.to_str().unwrap_or("");

        match parent {
            parent if parent == self.inode_allocator.root_inode => {
                if let Some(child_inode) = self.inode_index.find_child_by_name(parent, name_str) {
                    if let Some(attr) = self.get_attrs(child_inode) {
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
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

                    if let Some(child_inode) = self.inode_index.find_child_by_name(parent, name_str)
                    {
                        if let Some(attr) = self.get_attrs(child_inode) {
                            reply.entry(&TTL, &attr, 0);
                            return;
                        }
                    }
                }
            }
        }

        reply.error(ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        if let Some(attr) = self.get_attrs(ino) {
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
        match self.inode_index.get(ino) {
            Some(VFSEntry::TextFile(text_file)) => {
                let data = text_file.read(offset, size);
                reply.data(&data);
            }
            Some(VFSEntry::FHIRResource(resource)) => {
                let data = resource.read(offset, size);
                reply.data(&data);
            }
            _ => reply.error(ENOENT),
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
            ino if ino == self.inode_allocator.root_inode => {
                let mut listing = DirectoryListing::new();
                listing.add_current_dir(self.inode_allocator.root_inode);
                listing.add_parent_dir(self.inode_allocator.root_inode);

                // Add all children of root
                let children = self
                    .inode_index
                    .get_children(self.inode_allocator.root_inode);
                for child_inode in children {
                    if let Some(entry) = self.inode_index.get(child_inode) {
                        match entry {
                            VFSEntry::Directory(dir) => listing.add_dir(dir.inode, &dir.name),
                            VFSEntry::TextFile(file) => listing.add_file(file.inode, &file.name),
                            _ => {}
                        }
                    }
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
                    listing.add_parent_dir(self.inode_allocator.root_inode);

                    // Add all children of this directory
                    let children = self.inode_index.get_children(dir_inode);
                    let mut files: Vec<_> = children
                        .iter()
                        .filter_map(|&inode| {
                            if let Some(VFSEntry::FHIRResource(resource)) =
                                self.inode_index.get(inode)
                            {
                                Some((resource.filename.clone(), inode))
                            } else {
                                None
                            }
                        })
                        .collect();
                    files.sort_by_key(|(name, _)| name.clone());
                    for (name, inode) in files {
                        listing.add_file(inode, &name);
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
