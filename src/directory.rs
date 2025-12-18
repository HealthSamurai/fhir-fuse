use fuser::FileAttr;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Directory {
    pub inode: u64,
    #[allow(dead_code)]
    pub name: String,
}

impl Directory {
    pub fn new(inode: u64, name: impl Into<String>) -> Self {
        Self {
            inode,
            name: name.into(),
        }
    }

    pub fn get_attr(&self) -> FileAttr {
        let ts = SystemTime::now();
        FileAttr {
            ino: self.inode,
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: fuser::FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: 501,
            gid: 20,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }
}

pub struct DirectoryEntry {
    pub inode: u64,
    pub file_type: fuser::FileType,
    pub name: String,
}

impl DirectoryEntry {
    pub fn new_dir(inode: u64, name: impl Into<String>) -> Self {
        Self {
            inode,
            file_type: fuser::FileType::Directory,
            name: name.into(),
        }
    }

    pub fn new_file(inode: u64, name: impl Into<String>) -> Self {
        Self {
            inode,
            file_type: fuser::FileType::RegularFile,
            name: name.into(),
        }
    }
}

pub struct DirectoryListing {
    entries: Vec<DirectoryEntry>,
}

impl DirectoryListing {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_current_dir(&mut self, inode: u64) {
        self.entries.push(DirectoryEntry::new_dir(inode, "."));
    }

    pub fn add_parent_dir(&mut self, inode: u64) {
        self.entries.push(DirectoryEntry::new_dir(inode, ".."));
    }

    pub fn add_dir(&mut self, inode: u64, name: impl Into<String>) {
        self.entries.push(DirectoryEntry::new_dir(inode, name));
    }

    pub fn add_file(&mut self, inode: u64, name: impl Into<String>) {
        self.entries.push(DirectoryEntry::new_file(inode, name));
    }

    #[allow(dead_code)]
    pub fn add_entry(&mut self, entry: DirectoryEntry) {
        self.entries.push(entry);
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = &DirectoryEntry> {
        self.entries.iter()
    }

    pub fn into_vec(self) -> Vec<(u64, fuser::FileType, String)> {
        self.entries
            .into_iter()
            .map(|e| (e.inode, e.file_type, e.name))
            .collect()
    }
}
