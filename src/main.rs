use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};

mod text_file;
use text_file::TextFile;

mod directory;
use directory::{Directory, DirectoryListing};

mod inode_allocator;
use inode_allocator::InodeAllocator;

const TTL: Duration = Duration::from_secs(1);

const README_CONTENT: &str = include_str!("../assets/README.md");

#[derive(Debug, Deserialize, Serialize)]
struct FhirBundle {
    #[serde(rename = "resourceType")]
    resource_type: String,
    entry: Option<Vec<FhirBundleEntry>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FhirBundleEntry {
    resource: serde_json::Value,
}

#[derive(Debug, Clone)]
struct PatientFile {
    inode: u64,
    name: String,
    content: String,
}

struct FhirFuse {
    fhir_base_url: String,
    patients: HashMap<u64, PatientFile>,
    name_to_inode: HashMap<String, u64>,
    text_files: HashMap<u64, TextFile>,
    directories: HashMap<u64, Directory>,
    inode_allocator: InodeAllocator,
    root_inode: u64,
    patient_dir_inode: u64,
    readme_inode: u64,
}

impl FhirFuse {
    fn new(fhir_base_url: String) -> Self {
        let mut inode_allocator = InodeAllocator::new(1);

        let root_inode = inode_allocator.allocate();
        let patient_dir_inode = inode_allocator.allocate();
        let readme_inode = inode_allocator.allocate();

        let mut text_files = HashMap::new();
        let readme = TextFile::new(readme_inode, "README.md", README_CONTENT);
        text_files.insert(readme_inode, readme);

        let mut directories = HashMap::new();
        directories.insert(root_inode, Directory::new(root_inode, "/"));
        directories.insert(
            patient_dir_inode,
            Directory::new(patient_dir_inode, "Patient"),
        );

        let mut fs = FhirFuse {
            fhir_base_url: fhir_base_url.clone(),
            patients: HashMap::new(),
            name_to_inode: HashMap::new(),
            text_files,
            directories,
            inode_allocator,
            root_inode,
            patient_dir_inode,
            readme_inode,
        };
        if fhir_base_url != "offline" {
            fs.refresh_patients();
        }
        fs
    }

    fn refresh_patients(&mut self) {
        println!("Fetching patients from FHIR server...");

        match self.fetch_patients() {
            Ok(patients) => {
                self.patients.clear();
                self.name_to_inode.clear();

                for patient in patients {
                    let inode = self.inode_allocator.allocate();
                    let id = patient["id"].as_str().unwrap_or("unknown");
                    let filename = format!("{}.json", id);
                    let content = serde_json::to_string_pretty(&patient).unwrap_or_default();

                    let patient_file = PatientFile {
                        inode,
                        name: filename.clone(),
                        content,
                    };

                    self.patients.insert(inode, patient_file);
                    self.name_to_inode.insert(filename, inode);
                }

                println!("Loaded {} patients", self.patients.len());
            }
            Err(e) => {
                eprintln!("Failed to fetch patients: {:#?}", e);
            }
        }
    }

    fn fetch_patients(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        let url = format!("{}/Patient", self.fhir_base_url);
        println!("Url: {}", url);
        println!("Base URL: {}", self.fhir_base_url);
        let response = reqwest::blocking::get(&url)?;
        let bundle: FhirBundle = response.json()?;

        let patients = bundle
            .entry
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.resource)
            .collect();

        Ok(patients)
    }

    fn get_attrs(&self, inode: u64) -> Option<FileAttr> {
        if let Some(directory) = self.directories.get(&inode) {
            return Some(directory.get_attr());
        }

        let ts = SystemTime::now();

        match inode {
            _ => {
                if let Some(patient) = self.patients.get(&inode) {
                    Some(FileAttr {
                        ino: inode,
                        size: patient.content.len() as u64,
                        blocks: 1,
                        atime: ts,
                        mtime: ts,
                        ctime: ts,
                        crtime: ts,
                        kind: FileType::RegularFile,
                        perm: 0o644,
                        nlink: 1,
                        uid: 501,
                        gid: 20,
                        rdev: 0,
                        flags: 0,
                        blksize: 512,
                    })
                } else {
                    None
                }
            }
        }
    }
}

impl Filesystem for FhirFuse {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = name.to_str().unwrap_or("");

        match parent {
            parent if parent == self.root_inode => {
                if name_str == "Patient" {
                    if let Some(attr) = self.get_attrs(self.patient_dir_inode) {
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
                } else if let Some(text_file) =
                    self.text_files.values().find(|f| f.name == name_str)
                {
                    let attr = text_file.get_attr();
                    reply.entry(&TTL, &attr, 0);
                    return;
                }
            }
            parent if parent == self.patient_dir_inode => {
                if let Some(&inode) = self.name_to_inode.get(name_str) {
                    if let Some(attr) = self.get_attrs(inode) {
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
                }
            }
            _ => {}
        }

        reply.error(ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        if let Some(directory) = self.directories.get(&ino) {
            let attr = directory.get_attr();
            reply.attr(&TTL, &attr);
        } else if let Some(text_file) = self.text_files.get(&ino) {
            let attr = text_file.get_attr();
            reply.attr(&TTL, &attr);
        } else if let Some(attr) = self.get_attrs(ino) {
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
        if let Some(text_file) = self.text_files.get(&ino) {
            let data = text_file.read(offset, size);
            reply.data(&data);
        } else if let Some(patient) = self.patients.get(&ino) {
            let content = patient.content.as_bytes();
            let offset = offset as usize;
            let size = size as usize;

            if offset < content.len() {
                let end = std::cmp::min(offset + size, content.len());
                reply.data(&content[offset..end]);
            } else {
                reply.data(&[]);
            }
        } else {
            reply.error(ENOENT);
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
        match ino {
            ino if ino == self.root_inode => {
                let mut listing = DirectoryListing::new();
                listing.add_current_dir(self.root_inode);
                listing.add_parent_dir(self.root_inode);
                listing.add_dir(self.patient_dir_inode, "Patient");

                for text_file in self.text_files.values() {
                    listing.add_file(text_file.inode, &text_file.name);
                }

                let entries = listing.into_vec();
                for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                    if reply.add(entry.0, (i + 1) as i64, entry.1, &entry.2) {
                        break;
                    }
                }
                reply.ok();
            }
            ino if ino == self.patient_dir_inode => {
                let mut listing = DirectoryListing::new();
                listing.add_current_dir(self.patient_dir_inode);
                listing.add_parent_dir(self.root_inode);

                for patient in self.patients.values() {
                    listing.add_file(patient.inode, &patient.name);
                }

                let entries = listing.into_vec();
                for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                    if reply.add(entry.0, (i + 1) as i64, entry.1, &entry.2) {
                        break;
                    }
                }
                reply.ok();
            }
            _ => {
                reply.error(ENOENT);
            }
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
        MountOption::RO,
        MountOption::FSName("fhir-fuse".to_string()),
    ];

    match fuser::mount2(fs, mountpoint, &options) {
        Ok(_) => println!("Filesystem unmounted"),
        Err(e) => eprintln!("Failed to mount filesystem: {}", e),
    }
}
