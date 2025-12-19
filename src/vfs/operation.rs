use std::collections::HashMap;
use std::time::Instant;

/// Represents an operation path like /ViewDefinition/$run
#[derive(Debug, Clone)]
pub struct OperationPath {
    pub inode: u64,
    pub resource_type: String,
    pub operation_name: String, // e.g., "run", "validate", "expand"
    pub path: String,           // e.g., "$run"
}

impl OperationPath {
    pub fn new(inode: u64, resource_type: String, operation_name: String) -> Self {
        let path = format!("${}", operation_name);
        Self {
            inode,
            resource_type,
            operation_name,
            path,
        }
    }

    pub fn get_attr_with_ownership(&self, uid: u32, gid: u32) -> fuser::FileAttr {
        let ts = std::time::SystemTime::now();
        fuser::FileAttr {
            ino: self.inode,
            size: 4096,
            blocks: 1,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: fuser::FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid,
            gid,
            rdev: 0,
            flags: 0,
            blksize: 4096,
        }
    }
}

/// Represents an operation execution for a specific resource
#[derive(Debug, Clone)]
pub struct OperationExecution {
    pub inode: u64,
    pub resource_type: String,
    pub resource_id: String,
    pub operation_name: String,
    pub format: String,         // e.g., "json", "csv"
    pub path: String,           // e.g., "view-def-1.json"
    pub parent_inode: u64,      // The operation path inode
    pub result: Option<String>, // Cached result
    pub last_executed: Option<Instant>,
}

impl OperationExecution {
    pub fn new(
        inode: u64,
        resource_type: String,
        resource_id: String,
        operation_name: String,
        format: String,
        parent_inode: u64,
    ) -> Self {
        let path = format!("{}.{}", resource_id, format);
        Self {
            inode,
            resource_type,
            resource_id,
            operation_name,
            format,
            path,
            parent_inode,
            result: None,
            last_executed: None,
        }
    }

    pub fn parse_filename(filename: &str) -> Option<(String, String)> {
        // Parse "resource-id.format" into (resource_id, format)
        if let Some(dot_pos) = filename.rfind('.') {
            if dot_pos > 0 && dot_pos < filename.len() - 1 {
                let resource_id = &filename[..dot_pos];
                let format = &filename[dot_pos + 1..];

                // Validate format - support json and csv
                if matches!(format, "json" | "csv") {
                    return Some((resource_id.to_string(), format.to_string()));
                }
            }
        }
        None
    }

    pub fn get_attr_with_ownership(&self, uid: u32, gid: u32) -> fuser::FileAttr {
        let ts = std::time::SystemTime::now();
        let size = self.result.as_ref().map(|s| s.len() as u64).unwrap_or(0);
        fuser::FileAttr {
            ino: self.inode,
            size,
            blocks: (size + 511) / 512,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: fuser::FileType::RegularFile,
            perm: 0o444, // Read-only
            nlink: 1,
            uid,
            gid,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }
}

/// Manages operation-related state and caching
#[derive(Debug)]
pub struct OperationManager {
    pub operation_paths: HashMap<u64, OperationPath>, // inode -> OperationPath
    pub operation_executions: HashMap<u64, OperationExecution>, // inode -> OperationExecution
    pub resource_operations: HashMap<String, Vec<String>>, // resource_type -> [operations]
}

impl OperationManager {
    pub fn new() -> Self {
        let mut resource_operations = HashMap::new();

        // Define supported operations for each resource type
        resource_operations.insert("ViewDefinition".to_string(), vec!["run".to_string()]);

        Self {
            operation_paths: HashMap::new(),
            operation_executions: HashMap::new(),
            resource_operations,
        }
    }

    pub fn supports_operation(&self, resource_type: &str, operation: &str) -> bool {
        self.resource_operations
            .get(resource_type)
            .map(|ops| ops.iter().any(|op| op == operation))
            .unwrap_or(false)
    }

    pub fn get_supported_operations(&self, resource_type: &str) -> Vec<String> {
        self.resource_operations
            .get(resource_type)
            .cloned()
            .unwrap_or_default()
    }

    pub fn add_operation_path(&mut self, path: OperationPath) {
        self.operation_paths.insert(path.inode, path);
    }

    pub fn add_operation_execution(&mut self, execution: OperationExecution) {
        self.operation_executions.insert(execution.inode, execution);
    }

    pub fn get_operation_path(&self, inode: u64) -> Option<&OperationPath> {
        self.operation_paths.get(&inode)
    }

    pub fn get_operation_execution(&self, inode: u64) -> Option<&OperationExecution> {
        self.operation_executions.get(&inode)
    }

    pub fn get_operation_execution_mut(&mut self, inode: u64) -> Option<&mut OperationExecution> {
        self.operation_executions.get_mut(&inode)
    }
}
