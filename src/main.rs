use fuser::{
    FileAttr, Filesystem, MountOption, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyWrite, Request,
};
use libc::{EACCES, EIO, ENODATA, ENOENT};
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
    pending_writes: HashMap<u64, Vec<u8>>, // Temporary storage for file writes
    created_files: HashMap<u64, (String, String)>, // inode -> (resource_type, filename)
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
            pending_writes: HashMap::new(),
            created_files: HashMap::new(),
        };
        // Don't load resources immediately - use lazy loading
        fs
    }

    fn ensure_resources_loaded(&mut self, resource_type: &str, force: bool) {
        if !self.loaded_resources.contains(resource_type) || force {
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
                    self.ensure_resources_loaded(&resource_type, true);

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
        if ino == self.inode_allocator.root_inode {
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
            return;
        }

        if let Some((resource_type, dir_inode)) = self
            .resource_directories
            .iter()
            .find(|(_, &dir_inode)| ino == dir_inode)
            .map(|(resource_type, &dir_inode)| (resource_type.clone(), dir_inode))
        {
            // Load resources on first access
            self.ensure_resources_loaded(&resource_type, true);

            let mut listing = DirectoryListing::new();
            listing.add_current_dir(dir_inode);
            listing.add_parent_dir(self.inode_allocator.root_inode);

            // Add all children of this directory
            let children = self.inode_index.get_children(dir_inode);
            let mut files: Vec<_> = children
                .iter()
                .filter_map(|&inode| {
                    if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(inode) {
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
            return;
        }

        reply.error(ENOENT);
    }

    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let name_str = name.to_str().unwrap_or("");

        // Check if this is a resource directory
        let matching_resource = self
            .resource_directories
            .iter()
            .find(|(_, &dir_inode)| parent == dir_inode)
            .map(|(resource_type, _)| resource_type.clone());

        if let Some(resource_type) = matching_resource {
            println!("=== File Creation ===");
            println!("Resource Type: {}", resource_type);
            println!("Filename: {}", name_str);

            if name_str.ends_with(".json") {
                let resource_id = name_str.trim_end_matches(".json");
                println!("Resource ID: {}", resource_id);
                println!("Full path: /{}/{}", resource_type, name_str);
            }

            // Allocate a new inode for this file
            let inode = self.inode_allocator.allocate();

            // Initialize empty content for this inode
            self.pending_writes.insert(inode, Vec::new());

            // Track the resource type and filename for this created file
            self.created_files
                .insert(inode, (resource_type.clone(), name_str.to_string()));

            // Create a FHIR resource entry and add to index
            let resource_entry = FHIRResource::new(
                inode,
                &resource_type,
                name_str.trim_end_matches(".json"),
                String::new(), // Empty content initially
            );
            self.inode_index.insert_resource(resource_entry);

            // Add parent-child relationship
            if let Some(&dir_inode) = self.resource_directories.get(&resource_type) {
                self.inode_index.add_parent_child_relation(dir_inode, inode);
            }

            // Create file attributes
            let ts = std::time::SystemTime::now();
            let attr = FileAttr {
                ino: inode,
                size: 0,
                blocks: 0,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: fuser::FileType::RegularFile,
                perm: 0o644,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };

            println!("Created file with inode: {}", inode);
            println!("=============================");

            // Return success with the file handle being the same as inode
            reply.created(&TTL, &attr, 0, inode, 0);
        } else {
            println!("=== File Creation Failed ===");
            println!("Can only create files in resource directories");
            println!("Parent inode: {}", parent);
            println!("Filename: {}", name_str);
            println!("=============================");
            reply.error(EACCES);
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        if let Some(content) = self.pending_writes.get_mut(&ino) {
            let offset = offset as usize;

            // Extend buffer if necessary
            if offset + data.len() > content.len() {
                content.resize(offset + data.len(), 0);
            }

            // Write data at the specified offset
            content[offset..offset + data.len()].copy_from_slice(data);

            println!("=== File Write ===");
            println!("Inode: {}", ino);
            println!("Offset: {}", offset);
            println!("Size: {} bytes", data.len());

            // Try to parse as JSON and print prettily
            if let Ok(text) = std::str::from_utf8(content) {
                println!("Content Preview:");
                println!("{}", text);

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    println!("Valid JSON detected:");
                    if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                        println!("{}", pretty);
                    }
                }
            }
            println!("==================");

            reply.written(data.len() as u32);
        } else {
            println!("Write attempt to unknown inode: {}", ino);
            reply.error(ENOENT);
        }
    }

    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        println!("=== File Flush ===");
        println!("Inode: {}", ino);

        // Check if this is a created file that needs to be pushed to the server
        if let Some((resource_type, filename)) = self.created_files.get(&ino) {
            if let Some(content) = self.pending_writes.get(&ino) {
                if let Ok(text) = std::str::from_utf8(content) {
                    println!("Final content ({} bytes):", content.len());
                    println!("{}", text);

                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                        println!("\nParsed as valid JSON:");
                        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                            println!("{}", pretty);
                        }

                        // Extract resource type and ID if present
                        if let Some(resource_type) =
                            json.get("resourceType").and_then(|v| v.as_str())
                        {
                            println!("\nResource Type: {}", resource_type);
                        }
                        if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                            println!("Resource ID: {}", id);
                        }
                    }

                    // Send to FHIR server
                    println!("\nPushing to FHIR server...");
                    match vfs::resource::put_to_fhir_server(
                        &self.fhir_base_url,
                        resource_type,
                        filename,
                        text,
                    ) {
                        Ok(_response) => {
                            println!("Successfully pushed to FHIR server");
                        }
                        Err(e) => {
                            println!("Failed to push to FHIR server: {}", e);
                        }
                    }
                }
            }
        } else if let Some(content) = self.pending_writes.get(&ino) {
            // For existing files that were modified
            if let Ok(text) = std::str::from_utf8(content) {
                println!("Final content ({} bytes):", content.len());
                println!("{}", text);

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    println!("\nParsed as valid JSON:");
                    if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                        println!("{}", pretty);
                    }

                    // Extract resource type and ID if present
                    if let Some(resource_type) = json.get("resourceType").and_then(|v| v.as_str()) {
                        println!("\nResource Type: {}", resource_type);
                    }
                    if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                        println!("Resource ID: {}", id);
                    }
                }
            }
        }

        println!("==================");
        reply.ok();
    }

    fn release(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        println!("=== File Release ===");
        println!("Inode: {}", ino);

        // Clean up pending writes for this inode
        if self.pending_writes.remove(&ino).is_some() {
            println!("Cleaned up pending writes for inode {}", ino);
        }

        // Clean up created files tracking
        if self.created_files.remove(&ino).is_some() {
            println!("Cleaned up created file tracking for inode {}", ino);
        }

        println!("====================");
        reply.ok();
    }

    fn listxattr(&mut self, _req: &Request, _ino: u64, _size: u32, reply: fuser::ReplyXattr) {
        // Return empty list of extended attributes
        // This tells macOS there are no xattrs, so cp won't try to copy them
        reply.size(0);
    }

    fn getxattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _name: &OsStr,
        _size: u32,
        reply: fuser::ReplyXattr,
    ) {
        // Extended attribute not found
        reply.error(ENODATA);
    }

    fn setxattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _name: &OsStr,
        _value: &[u8],
        _flags: i32,
        _position: u32,
        reply: ReplyEmpty,
    ) {
        // Silently ignore attempts to set extended attributes
        // This allows cp to succeed even if it tries to copy xattrs
        reply.ok();
    }

    fn setattr(
        &mut self,
        _req: &Request,
        ino: u64,
        mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        println!("=== Set Attributes ===");
        println!("Inode: {}", ino);
        if let Some(m) = mode {
            println!("Mode: {:o}", m);
        }
        if let Some(s) = size {
            println!("Size: {}", s);
        }

        // Get current attributes
        if let Some(mut attr) = self.get_attrs(ino) {
            // Update size if requested (for truncate operations)
            if let Some(new_size) = size {
                attr.size = new_size;

                // If this is a pending write, truncate the buffer
                if let Some(content) = self.pending_writes.get_mut(&ino) {
                    content.resize(new_size as usize, 0);
                    println!("Truncated pending write buffer to {} bytes", new_size);
                }
            }

            // Update mode if requested
            if let Some(new_mode) = mode {
                attr.perm = new_mode as u16;
            }

            println!("Updated attributes for inode {}", ino);
            println!("======================");
            reply.attr(&TTL, &attr);
        } else {
            println!("Inode {} not found", ino);
            println!("======================");
            reply.error(ENOENT);
        }
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        println!("=== File Unlink (Delete) ===");
        let name_str = name.to_str().unwrap_or("");
        println!("Parent inode: {}, Name: {}", parent, name_str);

        // Find the inode of the file to delete
        if let Some(file_inode) = self.inode_index.find_child_by_name(parent, name_str) {
            println!("Found file inode: {}", file_inode);

            // Determine if this is a resource file that needs server deletion
            let mut server_delete_needed = false;
            let mut resource_type = String::new();

            // Check if the parent is a resource directory
            for (res_type, &dir_inode) in &self.resource_directories {
                if dir_inode == parent {
                    resource_type = res_type.clone();
                    server_delete_needed = true;
                    break;
                }
            }

            // Delete from FHIR server if it's a resource file
            if server_delete_needed && name_str.ends_with(".json") {
                println!("Deleting resource from FHIR server...");
                match vfs::resource::delete_from_fhir_server(
                    &self.fhir_base_url,
                    &resource_type,
                    name_str,
                ) {
                    Ok(_) => {
                        println!("✓ Deleted from FHIR server");
                    }
                    Err(e) => {
                        println!("✗ Failed to delete from FHIR server: {}", e);
                        println!("======================");
                        reply.error(EIO);
                        return;
                    }
                }
            }

            // Remove from pending writes if present
            if self.pending_writes.remove(&file_inode).is_some() {
                println!("Removed from pending_writes");
            }

            // Remove from created files if present
            if self.created_files.remove(&file_inode).is_some() {
                println!("Removed from created_files");
            }

            // Remove from inode index (this handles all the internal cleanup)
            self.inode_index.remove(file_inode);
            println!("Removed from inode index");

            println!("✓ File deleted successfully");
            println!("======================");
            reply.ok();
        } else {
            println!("File not found");
            println!("======================");
            reply.error(ENOENT);
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
        MountOption::RW,
        MountOption::FSName("fhir-fuse".to_string()),
    ];

    match fuser::mount2(fs, mountpoint, &options) {
        Ok(_) => println!("Filesystem unmounted"),
        Err(e) => eprintln!("Failed to mount filesystem: {}", e),
    }
}
