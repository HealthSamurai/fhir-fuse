use fuser::{
    FileAttr, Filesystem, MountOption, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyWrite, Request,
};
use libc::{EACCES, EIO, ENODATA, ENOENT};
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

mod vfs;
use vfs::{
    Directory, DirectoryListing, FHIRResource, IndexStats, InodeIndex, Search, TextFile, VFSEntry,
};

mod fhir;
use fhir::{delete_from_fhir_server, fetch_capability_statement, fetch_resources_parallel, put_to_fhir_server};

mod inode_allocator;
use inode_allocator::InodeAllocator;

const TTL: Duration = Duration::from_secs(30);
const CACHE_DURATION: Duration = Duration::from_secs(5);

const README_CONTENT: &str = include_str!("../assets/README.md");
const SEARCH_README_CONTENT: &str = include_str!("../assets/SEARCH_README.md");

struct FhirFuse {
    fhir_base_url: String,
    http_client: Client,
    runtime: Arc<Runtime>,
    inode_index: InodeIndex,
    resource_directories: HashMap<String, u64>,
    search_directories: HashMap<u64, u64>,
    loaded_resources: HashSet<String>,
    resource_load_times: HashMap<String, std::time::Instant>,
    inode_allocator: InodeAllocator,
    pending_writes: HashMap<u64, Vec<u8>>,
    created_files: HashMap<u64, (String, String)>,
    temp_files: HashMap<u64, (u64, String, Vec<u8>)>,
    lookup_counter: u64,
    readdir_counter: u64,
}

impl FhirFuse {
    fn new(fhir_base_url: String, http_client: Client, runtime: Arc<Runtime>) -> Self {
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

        // Fetch capabilities async and create directories for each resource type
        let caps_result = runtime.block_on(fetch_capability_statement(&http_client, &fhir_base_url));

        match caps_result {
            Ok(caps) => {
                println!("Successfully fetched capabilities: {} resource types", caps.resources.len());
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

        FhirFuse {
            fhir_base_url,
            http_client,
            runtime,
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
        }
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
            self.refresh_resources(resource_type);
            self.loaded_resources.insert(resource_type.to_string());
            self.resource_load_times
                .insert(resource_type.to_string(), std::time::Instant::now());
        }
    }

    fn refresh_resources(&mut self, resource_type: &str) {
        let client = self.http_client.clone();
        let base_url = self.fhir_base_url.clone();
        let rt = resource_type.to_string();

        let result = self.runtime.block_on(async {
            fetch_resources_parallel(&client, &base_url, &rt).await
        });

        match result {
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

                    if let Some(dir) = dir_inode {
                        self.inode_index.add_parent_child_relation(dir, inode);
                    }
                    count += 1;
                }

                println!("[FHIR] Loaded {} {} resources", count, resource_type);
            }
            Err(e) => {
                println!("[FHIR] Failed to fetch {}: {}", resource_type, e);
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
                let matching_resource = self
                    .resource_directories
                    .iter()
                    .find(|(_, &dir_inode)| parent == dir_inode)
                    .map(|(resource_type, _)| resource_type.clone());

                if let Some(resource_type) = matching_resource {
                    self.ensure_resources_loaded(&resource_type, false);

                    if let Some(child_inode) = self.inode_index.find_child_by_name(parent, name_str)
                    {
                        if let Some(attr) = self.get_attrs(child_inode) {
                            reply.entry(&TTL, &attr, 0);
                            return;
                        }
                    }
                }

                if self.search_directories.contains_key(&parent) {
                    if let Some(child_inode) = self.inode_index.find_child_by_name(parent, name_str)
                    {
                        if let Some(attr) = self.get_attrs(child_inode) {
                            reply.entry(&TTL, &attr, 0);
                            return;
                        }
                    }
                }

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

        if self.search_directories.contains_key(&ino) {
            let mut listing = DirectoryListing::new();
            listing.add_current_dir(ino);

            if let Some(VFSEntry::Search(search)) = self.inode_index.get(ino) {
                listing.add_parent_dir(search.parent_inode);
            }

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

        if let Some((resource_type, dir_inode)) = self
            .resource_directories
            .iter()
            .find(|(_, &dir_inode)| ino == dir_inode)
            .map(|(resource_type, &dir_inode)| (resource_type.clone(), dir_inode))
        {
            self.ensure_resources_loaded(&resource_type, false);

            let mut listing = DirectoryListing::new();
            listing.add_current_dir(dir_inode);
            listing.add_parent_dir(self.inode_allocator.root_inode);

            let children = self.inode_index.get_children(dir_inode);

            for &child_inode in &children {
                if let Some(VFSEntry::Search(search)) = self.inode_index.get(child_inode) {
                    listing.add_dir(search.inode, &search.name);
                }
            }

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

        let matching_resource = self
            .resource_directories
            .iter()
            .find(|(_, &dir_inode)| parent == dir_inode)
            .map(|(resource_type, _)| resource_type.clone());

        let is_temp_file = |name: &str| -> bool {
            name.starts_with('.') && name.ends_with(".swp")
                || name.starts_with('.') && name.ends_with(".swo")
                || name.starts_with('.') && name.contains(".sw")
                || name == "4913"
                || name.starts_with(".")
                || name.ends_with("~")
        };

        if let Some(resource_type) = matching_resource {
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
                String::new(),
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
            reply.created(&TTL, &attr, 0, inode, 0);
        } else {
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
        if let Some((_parent, _filename, content)) = self.temp_files.get_mut(&ino) {
            let offset = offset as usize;
            if offset + data.len() > content.len() {
                content.resize(offset + data.len(), 0);
            }
            content[offset..offset + data.len()].copy_from_slice(data);
            reply.written(data.len() as u32);
            return;
        }

        if !self.pending_writes.contains_key(&ino) {
            if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(ino) {
                self.pending_writes.insert(ino, resource.content.as_bytes().to_vec());
            } else {
                self.pending_writes.insert(ino, Vec::new());
            }
        }

        if let Some(content) = self.pending_writes.get_mut(&ino) {
            let offset = offset as usize;

            if offset + data.len() > content.len() {
                content.resize(offset + data.len(), 0);
            }

            content[offset..offset + data.len()].copy_from_slice(data);

            reply.written(data.len() as u32);
        } else {
            reply.error(ENOENT);
        }
    }

    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        if self.temp_files.contains_key(&ino) {
            reply.ok();
            return;
        }

        if let Some(content) = self.pending_writes.get(&ino) {
            if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(ino) {
                if let Ok(text) = std::str::from_utf8(content) {
                    let is_new_file = self.created_files.contains_key(&ino);
                    let action = if is_new_file { "created" } else { "updated" };

                    let client = self.http_client.clone();
                    let base_url = self.fhir_base_url.clone();
                    let resource_type = resource.resource_type.clone();
                    let filename = resource.filename.clone();
                    let resource_id = resource.resource_id.clone();
                    let content_str = text.to_string();

                    let result = self.runtime.block_on(async {
                        put_to_fhir_server(&client, &base_url, &resource_type, &filename, &content_str).await
                    });

                    match result {
                        Ok(_response) => {
                            println!("[FHIR] {}: {} {}", resource_type, resource_id, action);
                            self.loaded_resources.remove(&resource_type);
                            self.resource_load_times.remove(&resource_type);
                        }
                        Err(e) => {
                            println!("[FHIR] {}: {} {} failed: {}", resource_type, resource_id, action, e);
                        }
                    }
                }
            }
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
        self.pending_writes.remove(&ino);
        self.created_files.remove(&ino);
        reply.ok();
    }

    fn listxattr(&mut self, _req: &Request, _ino: u64, _size: u32, reply: fuser::ReplyXattr) {
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
        if let Some((_, _, content)) = self.temp_files.get_mut(&ino) {
            if let Some(new_size) = size {
                content.resize(new_size as usize, 0);
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

        if let Some(mut attr) = self.get_attrs(ino) {
            if let Some(new_size) = size {
                attr.size = new_size;

                if let Some(content) = self.pending_writes.get_mut(&ino) {
                    content.resize(new_size as usize, 0);
                }
            }

            if let Some(new_mode) = mode {
                attr.perm = new_mode as u16;
            }

            reply.attr(&TTL, &attr);
        } else {
            reply.error(ENOENT);
        }
    }

    fn access(&mut self, _req: &Request, ino: u64, _mask: i32, reply: ReplyEmpty) {
        if self.inode_index.get(ino).is_some() || self.temp_files.contains_key(&ino) {
            reply.ok();
        } else {
            reply.error(ENOENT);
        }
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: fuser::ReplyStatfs) {
        reply.statfs(0, 0, 0, 0, 0, 512, 255, 0);
    }

    fn opendir(&mut self, _req: &Request, ino: u64, _flags: i32, reply: fuser::ReplyOpen) {
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
        if self.temp_files.contains_key(&ino) {
            reply.opened(0, 0);
            return;
        }

        match self.inode_index.get(ino) {
            Some(VFSEntry::TextFile(_)) | Some(VFSEntry::FHIRResource(_)) => {
                reply.opened(0, 0);
            }
            Some(_) => {
                reply.error(libc::EISDIR);
            }
            None => {
                reply.error(ENOENT);
            }
        }
    }

    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name_str = name.to_str().unwrap_or("");

        let temp_inode = self.temp_files.iter()
            .find(|(_, (p, n, _))| *p == parent && n == name_str)
            .map(|(&ino, _)| ino);

        if let Some(inode) = temp_inode {
            self.temp_files.remove(&inode);
            reply.ok();
            return;
        }

        let mut server_delete_needed = false;

        for (_res_type, &dir_inode) in &self.resource_directories {
            if dir_inode == parent {
                server_delete_needed = true;
                break;
            }
        }

        let file_inode = self.inode_index.find_child_by_name(parent, name_str);

        if let Some(inode) = file_inode {
            if server_delete_needed && name_str.ends_with(".json") {
                if let Some(VFSEntry::FHIRResource(resource)) = self.inode_index.get(inode) {
                    let resource_type = resource.resource_type.clone();
                    let filename = resource.filename.clone();
                    let resource_id = resource.resource_id.clone();

                    let client = self.http_client.clone();
                    let base_url = self.fhir_base_url.clone();

                    let result = self.runtime.block_on(async {
                        delete_from_fhir_server(&client, &base_url, &resource_type, &filename).await
                    });

                    match result {
                        Ok(_) => {
                            println!("[FHIR] {}: {} deleted", resource_type, resource_id);
                            self.loaded_resources.remove(&resource_type);
                            self.resource_load_times.remove(&resource_type);
                        }
                        Err(e) => {
                            println!("[FHIR] {}: {} delete failed: {}", resource_type, resource_id, e);
                            reply.error(EIO);
                            return;
                        }
                    }
                }
            }

            self.pending_writes.remove(&inode);
            self.created_files.remove(&inode);
            self.inode_index.remove(inode);

            reply.ok();
        } else {
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

    // Create tokio runtime
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    );

    // Create HTTP client
    let http_client = Client::new();

    let fs = FhirFuse::new(fhir_base_url.clone(), http_client, runtime);

    let options = vec![
        MountOption::RW,
        MountOption::FSName("fhir-fuse".to_string()),
        MountOption::CUSTOM("noappledouble".to_string()),
        MountOption::CUSTOM("noapplexattr".to_string()),
    ];

    match fuser::mount2(fs, mountpoint, &options) {
        Ok(_) => println!("Filesystem unmounted"),
        Err(e) => eprintln!("Failed to mount filesystem: {}", e),
    }
}
