use fuser::FileAttr;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct TextFile {
    pub inode: u64,
    pub filename: String,
    pub content: String,
}

impl TextFile {
    pub fn new(inode: u64, name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            inode,
            filename: name.into(),
            content: content.into(),
        }
    }

    pub fn get_attr_with_ownership(&self, uid: u32, gid: u32) -> FileAttr {
        let ts = SystemTime::now();
        let size = self.content.len() as u64;
        let blocks = (size + 511) / 512; // Calculate actual blocks needed
        FileAttr {
            ino: self.inode,
            size,
            blocks,
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
