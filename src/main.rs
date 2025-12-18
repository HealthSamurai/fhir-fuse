use fuser::{
    FileAttr, Filesystem, MountOption, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyWrite, Request,
};
use libc::{EACCES, EIO, ENODATA, ENOENT};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::time::Duration;

mod vfs;
use vfs::{
    Directory, DirectoryListing, FHIRResource, IndexStats, InodeIndex, Search, TextFile, VFSEntry,
};

mod fhir;
use fhir::{fetch_capability_statement, fetch_resources};

mod inode_allocator;
use inode_allocator::InodeAllocator;

const TTL: Duration = Duration::from_secs(30);  // Cache attributes for 30 seconds to reduce Finder polling
const CACHE_DURATION: Duration = Duration::from_secs(5); // Force refresh after 5 seconds

const README_CONTENT: &str = include_str!("../assets/README.md");
const SEARCH_README_CONTENT: &str = include_str!("../assets/SEARCH_README.md");

struct FhirFuse {
    fhir_base_url: String,
    inode_index: InodeIndex,
    resource_directories: HashMap<String, u64>, // resource_type -> directory inode
    search_directories: HashMap<u64, u64>,      // search_dir_inode -> readme_inode
    loaded_resources: HashSet<String>,          // track which resource types have been loaded
    resource_load_times: HashMap<String, std::time::Instant>, // track when resources were loaded
    inode_allocator: InodeAllocator,
    pending_writes: HashMap<u64, Vec<u8>>, // Temporary storage for file writes
    created_files: HashMap<u64, (String, String)>, // inode -> (resource_type, filename)
    temp_files: HashMap<u64, (u64, String, Vec<u8>)>, // inode -> (parent_inode, filename, content) for vim temp files
    lookup_counter: u64,                   // Track number of lookup calls
    readdir_counter: u64,                  // Track number of readdir calls
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

        // Add .metadata_never_index to prevent Spotlight indexing
        let spotlight_block_inode = inode_allocator.allocate();
        let spotlight_block = TextFile::new(spotlight_block_inode, ".metadata_never_index", "");
        inode_index.insert_text_file(spotlight_block);
        inode_index.add_parent_child_relation(root_inode, spotlight_block_inode);

        let mut resource_directories = HashMap::new();
        let mut search_directories = HashMap::new();

        // Fetch capabilities and create directories for each resource type
        match fetch_capability_statement(&fhir_base_url) {
            Ok(caps) => {
                println!("Successfully fetched capabilities");
                for resource_type in &caps.resources {
                    let dir_inode = inode_allocator.allocate();
                    inode_index.insert_directory(Directory::new(dir_inode, resource_type.clone()));
                    inode_index.add_parent_child_relation(root_inode, dir_inode);
                    resource_directories.insert(resource_type.clone(), dir_inode);

                    // Add _search directory for each resource type
                    let search_inode = inode_allocator.allocate();
                    inode_index.insert_search(Search::new(
                        search_inode,
                        resource_type.clone(),
                        dir_inode,
                    ));
                    inode_index.add_parent_child_relation(dir_inode, search_inode);

                    // Add README.md file inside _search directory
                    let search_readme_inode = inode_allocator.allocate();
                    inode_index.insert_text_file(TextFile::new(
                        search_readme_inode,
                        "README.md",
                        SEARCH_README_CONTENT,
                    ));
                    inode_index.add_parent_child_relation(search_inode, search_readme_inode);
                    search_directories.insert(search_inode, search_readme_inode);
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch capabilities: {:#?}", e);
            }
        }

        let fs = FhirFuse {
            fhir_base_url: fhir_base_url.to_string(),
            inode_index,
            resource_directories,
            search_directories,
            loaded_resources: HashSet::new(),
            resource_load_times: HashMap::new(),
            inode_allocator,
            pending_writes: HashMap::new(),
            created_files: HashMap::new(),
            temp_files: HashMap::new(),
            lookup_counter: 0,
            readdir_counter: 0,
        };
        // Don't load resources immediately - use lazy loading
        fs
    }

    fn ensure_resources_loaded(&mut self, resource_type: &str, force_refresh: bool) {
        let should_refresh = force_refresh
            || !self.loaded_resources.contains(resource_type)
            || self
                .resource_load_times
                .get(resource_type)
                .map(|t| t.elapsed() > CACHE_DURATION)
                .unwrap_or(true);

        if should_refresh {
            println!(
                "Cache expired or force refresh for {}, fetching from server...",
                resource_type
            );
            self.refresh_resources(resource_type);
            self.loaded_resources.insert(resource_type.to_string());
            self.resource_load_times
                .insert(resource_type.to_string(), std::time::Instant::now());
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
                println!("0: [{}]: refresh: failed: {}", resource_type, e);
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
        self.lookup_counter += 1;

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
                    self.ensure_resources_loaded(&resource_type, false);

                    if let Some(child_inode) = self.inode_index.find_child_by_name(parent, name_str)
                    {
                        if let Some(attr) = self.get_attrs(child_inode) {
                            reply.entry(&TTL, &attr, 0);
                            return;
                        }
                    }
                }

                // Check if parent is a _search directory
                if self.search_directories.contains_key(&parent) {
                    if let Some(child_inode) = self.inode_index.find_child_by_name(parent, name_str)
                    {
                        if let Some(attr) = self.get_attrs(child_inode) {
                            reply.entry(&TTL, &attr, 0);
                            return;
                        }
                    }
                }

                // Check for temp files (vim swap files, etc.)
                for (&inode, (temp_parent, filename, content)) in &self.temp_files {
                    if *temp_parent == parent && filename == name_str {
                        let ts = std::time::SystemTime::now();
                        let attr = FileAttr {
                            ino: inode,
                            size: content.len() as u64,
                            blocks: 1,
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
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
                }
            }
        }

        reply.error(ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        match self.inode_index.get(ino) {
            Some(VFSEntry::FHIRResource(_)) | Some(VFSEntry::Directory(_)) | Some(VFSEntry::TextFile(_)) | Some(VFSEntry::Search(_)) => {
                if let Some(attr) = self.get_attrs(ino) {
                    reply.attr(&TTL, &attr);
                } else {
                    reply.error(ENOENT);
                }
            }
            None => {
                // Check temp files
                if let Some((_, _, content)) = self.temp_files.get(&ino) {
                    let ts = std::time::SystemTime::now();
                    let attr = FileAttr {
                        ino,
                        size: content.len() as u64,
                        blocks: 1,
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
                    reply.attr(&TTL, &attr);
                } else {
                    reply.error(ENOENT);
                }
            }
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
        _lock_owner: Option<u64>,
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
            _ => {
                // Check temp files
                if let Some((_, _filename, content)) = self.temp_files.get(&ino) {
                    let offset = offset as usize;
                    let size = size as usize;
                    let data = if offset < content.len() {
                        let end = std::cmp::min(offset + size, content.len());
                        content[offset..end].to_vec()
                    } else {
                        vec![]
                    };
                    reply.data(&data);
                    return;
                }
                reply.error(ENOENT);
            }
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
        self.readdir_counter += 1;
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
                        VFSEntry::TextFile(file) => listing.add_file(file.inode, &file.filename),
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

        // Check if this is a _search directory first
        if self.search_directories.contains_key(&ino) {
            let mut listing = DirectoryListing::new();
            listing.add_current_dir(ino);

            // Find parent directory (the resource type directory)
            if let Some(VFSEntry::Search(search)) = self.inode_index.get(ino) {
                listing.add_parent_dir(search.parent_inode);
            }

            // Add README.md file
            let children = self.inode_index.get_children(ino);
            for &child_inode in &children {
                if let Some(VFSEntry::TextFile(file)) = self.inode_index.get(child_inode) {
                    listing.add_file(file.inode, &file.filename);
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

        // Then check if it's a resource directory
        if let Some((resource_type, dir_inode)) = self
            .resource_directories
            .iter()
            .find(|(_, &dir_inode)| ino == dir_inode)
            .map(|(resource_type, &dir_inode)| (resource_type.clone(), dir_inode))
        {
            // Load resources on first access (don't force refresh if already loaded)
            self.ensure_resources_loaded(&resource_type, false);

            let mut listing = DirectoryListing::new();
            listing.add_current_dir(dir_inode);
            listing.add_parent_dir(self.inode_allocator.root_inode);

            // Add all children of this directory
            let children = self.inode_index.get_children(dir_inode);

            // Add _search directory
            for &child_inode in &children {
                if let Some(VFSEntry::Search(search)) = self.inode_index.get(child_inode) {
                    listing.add_dir(search.inode, &search.name);
                }
            }

            // Add resource files
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

        // Helper to check if this is a vim temp file
        let is_temp_file = |name: &str| -> bool {
            name.starts_with('.') && name.ends_with(".swp")
                || name.starts_with('.') && name.ends_with(".swo")
                || name.starts_with('.') && name.contains(".sw")
                || name == "4913"  // vim's test file
                || name.starts_with(".")  // other hidden files
                || name.ends_with("~")  // vim backup files
        };

        if let Some(resource_type) = matching_resource {
            // Check if this is a temp file (vim swap, test file, etc.)
            if is_temp_file(name_str) {
                let inode = self.inode_allocator.allocate();
                self.temp_files.insert(inode, (parent, name_str.to_string(), Vec::new()));

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

                println!("[create]: temp file {}", name_str);
                reply.created(&TTL, &attr, 0, inode, 0);
                return;
            }

            let inode = self.inode_allocator.allocate();
            self.pending_writes.insert(inode, Vec::new());
            self.created_files
                .insert(inode, (resource_type.clone(), name_str.to_string()));
            let resource_entry = FHIRResource::new(
                inode,
                &resource_type,
                name_str.trim_end_matches(".json"),
                String::new(), // Empty content initially
            );
            self.inode_index.insert_resource(resource_entry);

            if let Some(&dir_inode) = self.resource_directories.get(&resource_type) {
                self.inode_index.add_parent_child_relation(dir_inode, inode);
            }

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

            println!("[create]: {}/{}", resource_type, name_str);

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
        // Check if this is a temp file
        if let Some((_parent, _filename, content)) = self.temp_files.get_mut(&ino) {
            let offset = offset as usize;
            if offset + data.len() > content.len() {
                content.resize(offset + data.len(), 0);
            }
            content[offset..offset + data.len()].copy_from_slice(data);
            reply.written(data.len() as u32);
            return;
        }

        // Initialize pending_writes for this inode if it doesn't exist
        // This handles both newly created files and existing files being edited
        if !self.pending_writes.contains_key(&ino) {
            // For existing FHIR resources, load their current content
            if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(ino) {
                self.pending_writes.insert(ino, resource.content.as_bytes().to_vec());
            } else {
                // For new files, start with empty buffer
                self.pending_writes.insert(ino, Vec::new());
            }
        }

        if let Some(content) = self.pending_writes.get_mut(&ino) {
            let offset = offset as usize;

            // Extend buffer if necessary
            if offset + data.len() > content.len() {
                content.resize(offset + data.len(), 0);
            }

            // Write data at the specified offset
            content[offset..offset + data.len()].copy_from_slice(data);

            reply.written(data.len() as u32);
        } else {
            reply.error(ENOENT);
        }
    }

    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        // Skip temp files - they don't need to be synced to FHIR
        if self.temp_files.contains_key(&ino) {
            reply.ok();
            return;
        }

        // Check if we have pending writes for this inode
        if let Some(content) = self.pending_writes.get(&ino) {
            if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(ino) {
                // Update the resource content and push to server
                if let Ok(text) = std::str::from_utf8(content) {
                    let updated_resource = FHIRResource::new(
                        resource.inode,
                        &resource.resource_type,
                        &resource.resource_id,
                        text,
                    );

                    let is_new_file = self.created_files.contains_key(&ino);
                    let action = if is_new_file { "created" } else { "updated" };

                    match updated_resource.put_to_fhir_server(&self.fhir_base_url) {
                        Ok(_response) => {
                            println!("[FHIR] {}: {} {}", resource.resource_type, resource.resource_id, action);
                            // Invalidate cache for this resource type after successful write
                            self.loaded_resources.remove(&resource.resource_type);
                            self.resource_load_times.remove(&resource.resource_type);
                        }
                        Err(e) => {
                            println!("[FHIR] {}: {} {} failed: {}", resource.resource_type, resource.resource_id, action, e);
                        }
                    }
                }
            }
            // Non-FHIR resources are silently ignored
        }
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
        // Clean up pending writes if they exist
        self.pending_writes.remove(&ino);

        // Clean up created files tracking
        self.created_files.remove(&ino);

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

        // Check temp files first
        if let Some((_, _, content)) = self.temp_files.get_mut(&ino) {
            if let Some(new_size) = size {
                content.resize(new_size as usize, 0);
                println!("{}: setattr temp: truncate to {}", ino, new_size);
            }
            let ts = std::time::SystemTime::now();
            let attr = FileAttr {
                ino,
                size: content.len() as u64,
                blocks: 1,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: fuser::FileType::RegularFile,
                perm: mode.map(|m| m as u16).unwrap_or(0o644),
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };
            reply.attr(&TTL, &attr);
            return;
        }

        // Get current attributes
        if let Some(mut attr) = self.get_attrs(ino) {
            // Update size if requested (for truncate operations)
            if let Some(new_size) = size {
                attr.size = new_size;

                // If this is a pending write, truncate the buffer
                if let Some(content) = self.pending_writes.get_mut(&ino) {
                    content.resize(new_size as usize, 0);
                    println!("{}: setattr: truncate to {}", ino, new_size);
                }
            }

            // Update mode if requested
            if let Some(new_mode) = mode {
                attr.perm = new_mode as u16;
            }

            reply.attr(&TTL, &attr);
        } else {
            println!("{}: setattr: not found", ino);
            reply.error(ENOENT);
        }
    }

    fn access(&mut self, _req: &Request, ino: u64, _mask: i32, reply: ReplyEmpty) {
        // Check if inode exists (including temp files)
        if self.inode_index.get(ino).is_some() || self.temp_files.contains_key(&ino) {
            reply.ok();
        } else {
            reply.error(ENOENT);
        }
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: fuser::ReplyStatfs) {
        reply.statfs(
            0,    // blocks
            0,    // bfree
            0,    // bavail
            0,    // files
            0,    // ffree
            512,  // bsize
            255,  // namelen
            0,    // frsize
        );
    }

    fn opendir(&mut self, _req: &Request, ino: u64, _flags: i32, reply: fuser::ReplyOpen) {
        // Check if inode exists and is a directory
        match self.inode_index.get(ino) {
            Some(VFSEntry::Directory(_)) | Some(VFSEntry::Search(_)) => {
                reply.opened(0, 0);
            }
            Some(_) => {
                reply.error(libc::ENOTDIR);
            }
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: fuser::ReplyOpen) {
        // Check temp files first
        if self.temp_files.contains_key(&ino) {
            reply.opened(0, 0);
            return;
        }

        // Check if inode exists and is a readable file
        match self.inode_index.get(ino) {
            Some(VFSEntry::TextFile(_)) | Some(VFSEntry::FHIRResource(_)) => {
                // Use direct I/O flag to prevent kernel caching issues
                // Return 0 as the file handle - we identify files by inode
                reply.opened(0, 0);
            }
            Some(_) => {
                // Directory or other non-file type
                reply.error(libc::EISDIR);
            }
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name_str = name.to_str().unwrap_or("");

        // Check if this is a temp file first
        let temp_inode = self.temp_files.iter()
            .find(|(_, (p, n, _))| *p == parent && n == name_str)
            .map(|(&ino, _)| ino);

        if let Some(inode) = temp_inode {
            self.temp_files.remove(&inode);
            reply.ok();
            return;
        }

        // Check if this is a resource directory and might need server deletion
        let mut server_delete_needed = false;

        for (_res_type, &dir_inode) in &self.resource_directories {
            if dir_inode == parent {
                server_delete_needed = true;
                break;
            }
        }

        // Find the inode of the file to delete (if it exists in our index)
        let file_inode = self.inode_index.find_child_by_name(parent, name_str);

        if let Some(inode) = file_inode {
            // Delete from FHIR server if it's a resource file
            if server_delete_needed && name_str.ends_with(".json") {
                // Try to get the FHIRResource from the index and use its method
                if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(inode) {
                    let resource_type = resource.resource_type.clone();
                    println!("[unlink]: {}/{}", resource_type, name_str);
                    match resource.delete_from_fhir_server(&self.fhir_base_url) {
                        Ok(_) => {
                            self.loaded_resources.remove(&resource_type);
                            self.resource_load_times.remove(&resource_type);
                            println!("Invalidated cache for resource type: {}", resource_type);
                        }
                        Err(e) => {
                            println!(
                                "[unlink]: {}/{}: FHIR delete failed: {}",
                                resource_type, name_str, e
                            );
                            reply.error(EIO);
                            return;
                        }
                    }
                } else {
                    println!("{}: [{}]: unlink: not a FHIR resource", inode, name_str);
                }
            }

            // Remove from pending writes if present
            self.pending_writes.remove(&inode);

            // Remove from created_files if present
            self.created_files.remove(&inode);

            // Remove from the index
            self.inode_index.remove(inode);

            println!("{}: [{}]: unlink: deleted", inode, name_str);
            reply.ok();
        } else {
            println!("0: [{}]: unlink: not found in parent {}", name_str, parent);
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
        MountOption::CUSTOM("noappledouble".to_string()), // Disable ._* files
        MountOption::CUSTOM("noapplexattr".to_string()),  // Disable Apple extended attributes
    ];

    match fuser::mount2(fs, mountpoint, &options) {
        Ok(_) => println!("Filesystem unmounted"),
        Err(e) => eprintln!("Failed to mount filesystem: {}", e),
    }
}
