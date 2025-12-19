use fuser::FileAttr;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FHIRResource {
    pub inode: u64,
    pub resource_type: String,
    pub resource_id: String,
    pub filename: String,
    pub content: String,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone)]
pub struct ResourceVersion {
    pub inode: u64,
    pub resource_type: String,
    pub resource_id: String,
    pub version_id: String,
    pub filename: String,
    pub content: String,
}

impl FHIRResource {
    pub fn new(
        inode: u64,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let resource_id = resource_id.into();
        let filename = format!("{}.json", resource_id);
        Self {
            inode,
            resource_type: resource_type.into(),
            resource_id,
            filename,
            content: content.into(),
            mtime: SystemTime::now(),
        }
    }

    pub fn get_attr_with_ownership(&self, uid: u32, gid: u32) -> FileAttr {
        let size = self.content.len() as u64;
        let blocks = (size + 511) / 512; // Calculate actual blocks needed
        FileAttr {
            ino: self.inode,
            size,
            blocks,
            atime: self.mtime,
            mtime: self.mtime,
            ctime: self.mtime,
            crtime: self.mtime,
            kind: fuser::FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid,
            gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }

    pub fn read(&self, offset: i64, size: u32) -> Vec<u8> {
        let content = self.content.as_bytes();
        let offset = offset as usize;
        let size = size as usize;

        if offset < content.len() {
            let end = std::cmp::min(offset + size, content.len());
            content[offset..end].to_vec()
        } else {
            vec![]
        }
    }
}

impl ResourceVersion {
    pub fn new(
        inode: u64,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        version_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let resource_type = resource_type.into();
        let resource_id = resource_id.into();
        let version_id = version_id.into();
        let filename = format!("{}.json", version_id);

        Self {
            inode,
            resource_type,
            resource_id,
            version_id,
            filename,
            content: content.into(),
        }
    }

    pub fn get_attr_with_ownership(&self, uid: u32, gid: u32) -> FileAttr {
        let ts = SystemTime::now();
        let size = self.content.len() as u64;
        FileAttr {
            ino: self.inode,
            size,
            blocks: (size + 511) / 512,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: fuser::FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid,
            gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }
}
