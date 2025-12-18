use fuser::FileAttr;
use std::time::SystemTime;

/// Represents a search query directory inside _search/
/// e.g., _search/name=John&birthdate=1990/
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub inode: u64,
    pub query: String,           // The query string (directory name)
    pub resource_type: String,   // The parent resource type
    pub parent_inode: u64,       // The _search directory inode
    pub created_at: SystemTime,
}

impl SearchQuery {
    pub fn new(inode: u64, query: String, resource_type: String, parent_inode: u64) -> Self {
        Self {
            inode,
            query,
            resource_type,
            parent_inode,
            created_at: SystemTime::now(),
        }
    }

    pub fn get_attr(&self) -> FileAttr {
        FileAttr {
            ino: self.inode,
            size: 4096,
            blocks: 1,
            atime: self.created_at,
            mtime: self.created_at,
            ctime: self.created_at,
            crtime: self.created_at,
            kind: fuser::FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: 501,
            gid: 20,
            rdev: 0,
            flags: 0,
            blksize: 4096,
        }
    }
}

/// Represents a resource type group inside a search query result
/// e.g., _search/name=John/Patient/
#[derive(Debug, Clone)]
pub struct SearchResultGroup {
    pub inode: u64,
    pub resource_type: String,   // The grouped resource type (e.g., "Patient")
    pub parent_inode: u64,       // The SearchQuery inode
    pub created_at: SystemTime,
}

impl SearchResultGroup {
    pub fn new(inode: u64, resource_type: String, parent_inode: u64) -> Self {
        Self {
            inode,
            resource_type,
            parent_inode,
            created_at: SystemTime::now(),
        }
    }

    pub fn get_attr(&self) -> FileAttr {
        FileAttr {
            ino: self.inode,
            size: 4096,
            blocks: 1,
            atime: self.created_at,
            mtime: self.created_at,
            ctime: self.created_at,
            crtime: self.created_at,
            kind: fuser::FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: 501,
            gid: 20,
            rdev: 0,
            flags: 0,
            blksize: 4096,
        }
    }
}
