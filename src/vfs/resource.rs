use crate::fhir;
use fuser::FileAttr;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FHIRResource {
    pub inode: u64,
    pub resource_type: String,
    #[allow(dead_code)]
    pub resource_id: String,
    pub filename: String,
    pub content: String,
    pub mtime: SystemTime,
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

    pub fn get_attr(&self) -> FileAttr {
        FileAttr {
            ino: self.inode,
            size: self.content.len() as u64,
            blocks: 1,
            atime: self.mtime,
            mtime: self.mtime,
            ctime: self.mtime,
            crtime: self.mtime,
            kind: fuser::FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: 501,
            gid: 20,
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

    pub fn put_to_fhir_server(&self, fhir_base_url: &str) -> anyhow::Result<String> {
        fhir::put_to_fhir_server(
            fhir_base_url,
            &self.resource_type,
            &self.filename,
            &self.content,
        )
    }

    pub fn delete_from_fhir_server(&self, fhir_base_url: &str) -> anyhow::Result<()> {
        fhir::delete_from_fhir_server(fhir_base_url, &self.resource_type, &self.filename)
    }
}
