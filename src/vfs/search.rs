use fuser::FileAttr;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct SearchPath {
    pub inode: u64,
    pub path: String,
    pub display_path: String,
    #[allow(dead_code)]
    pub resource_type: String,
    pub parent_inode: u64,
}

impl SearchPath {
    pub fn new(inode: u64, resource_type: String, parent_inode: u64) -> Self {
        Self {
            inode,
            path: "_search".to_string(),
            display_path: format!("/{}/_search", resource_type).to_string(),
            resource_type,
            parent_inode,
        }
    }

    pub fn get_attr(&self) -> FileAttr {
        FileAttr {
            ino: self.inode,
            size: 4096,
            blocks: 1,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
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

    #[allow(dead_code)]
    pub fn get_name(&self) -> &str {
        &self.path
    }
}
