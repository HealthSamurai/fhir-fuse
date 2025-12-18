use fuser::FileAttr;
use std::time::SystemTime;

pub fn put_to_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
    content: &str,
) -> anyhow::Result<String> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    println!("Sending resource to FHIR server:");
    println!("  Method: PUT");
    println!("  URL: {}", url);
    println!("  Resource Type: {}", resource_type);
    println!("  Resource ID: {}", resource_id);
    println!("  Content size: {} bytes", content.len());

    let client = reqwest::blocking::Client::new();
    let response = client
        .put(&url)
        .header("Content-Type", "application/fhir+json")
        .body(content.to_string())
        .send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!("  Response status: {}", status);

    if status.is_success() {
        println!("  ✓ Successfully sent to FHIR server (PUT)");
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                println!("  Resource ID in response: {}", id);
            }
        }
        Ok(response_text)
    } else {
        println!("  ✗ Failed to send to FHIR server");
        println!("  Error response: {}", response_text);
        Err(anyhow::anyhow!(
            "Failed to PUT resource to FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}

pub fn delete_from_fhir_server(
    fhir_base_url: &str,
    resource_type: &str,
    filename: &str,
) -> anyhow::Result<()> {
    let resource_id = filename.trim_end_matches(".json");
    let url = format!("{}/{}/{}", fhir_base_url, resource_type, resource_id);

    println!("Deleting resource from FHIR server:");
    println!("  Method: DELETE");
    println!("  URL: {}", url);
    println!("  Resource Type: {}", resource_type);
    println!("  Resource ID: {}", resource_id);

    let client = reqwest::blocking::Client::new();
    let response = client.delete(&url).send()?;

    let status = response.status();
    let response_text = response.text()?;

    println!("  Response status: {}", status);

    if status.is_success() || status.as_u16() == 404 {
        println!("  ✓ Successfully deleted from FHIR server");
        Ok(())
    } else {
        println!("  ✗ Failed to delete from FHIR server");
        println!("  Error response: {}", response_text);
        Err(anyhow::anyhow!(
            "Failed to DELETE resource from FHIR server: HTTP {} - {}",
            status,
            response_text
        ))
    }
}

#[derive(Debug, Clone)]
pub struct FHIRResource {
    pub inode: u64,
    pub resource_type: String,
    #[allow(dead_code)]
    pub resource_id: String,
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
        }
    }

    pub fn get_attr(&self) -> FileAttr {
        let ts = SystemTime::now();
        FileAttr {
            ino: self.inode,
            size: self.content.len() as u64,
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
