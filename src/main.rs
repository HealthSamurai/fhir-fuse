use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::time::{Duration, SystemTime};

const TTL: Duration = Duration::from_secs(1);

const ROOT_INODE: u64 = 1;
const PATIENT_DIR_INODE: u64 = 2;
const PATIENT_FILE_INODE_START: u64 = 100;

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
}

impl FhirFuse {
    fn new(fhir_base_url: String) -> Self {
        let mut fs = FhirFuse {
            fhir_base_url: fhir_base_url.clone(),
            patients: HashMap::new(),
            name_to_inode: HashMap::new(),
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
                println!("{:#?}", &patients);
                self.patients.clear();
                self.name_to_inode.clear();

                for (idx, patient) in patients.into_iter().enumerate() {
                    let inode = PATIENT_FILE_INODE_START + idx as u64;
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
        let ts = SystemTime::now();

        match inode {
            ROOT_INODE => Some(FileAttr {
                ino: ROOT_INODE,
                size: 0,
                blocks: 0,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            }),
            PATIENT_DIR_INODE => Some(FileAttr {
                ino: PATIENT_DIR_INODE,
                size: 0,
                blocks: 0,
                atime: ts,
                mtime: ts,
                ctime: ts,
                crtime: ts,
                kind: FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            }),
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
            ROOT_INODE => {
                if name_str == "Patient" {
                    if let Some(attr) = self.get_attrs(PATIENT_DIR_INODE) {
                        reply.entry(&TTL, &attr, 0);
                        return;
                    }
                }
            }
            PATIENT_DIR_INODE => {
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
        if let Some(attr) = self.get_attrs(ino) {
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
        if let Some(patient) = self.patients.get(&ino) {
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
            ROOT_INODE => {
                let entries = vec![
                    (ROOT_INODE, FileType::Directory, "."),
                    (ROOT_INODE, FileType::Directory, ".."),
                    (PATIENT_DIR_INODE, FileType::Directory, "Patient"),
                ];

                for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                    if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                        break;
                    }
                }
                reply.ok();
            }
            PATIENT_DIR_INODE => {
                let mut entries = vec![
                    (PATIENT_DIR_INODE, FileType::Directory, ".".to_string()),
                    (ROOT_INODE, FileType::Directory, "..".to_string()),
                ];

                for patient in self.patients.values() {
                    entries.push((patient.inode, FileType::RegularFile, patient.name.clone()));
                }

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
