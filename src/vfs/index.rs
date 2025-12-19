use super::directory::Directory;
use super::operation::{OperationExecution, OperationPath};
use super::resource::{FHIRResource, ResourceVersion};
use super::search::SearchPath;
use super::search_query::{SearchQuery, SearchResultGroup};
use super::text_file::TextFile;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum VFSEntry {
    Directory(Directory),             // /Patient
    TextFile(TextFile),               // /README.md
    FHIRResource(FHIRResource),       // /Patient/pt-1.json
    ResourceVersion(ResourceVersion), // /Patient/.pt-1/Patient-pt-1-version-1.json
    SearchPath(SearchPath),           // /Patient/_search
    SearchQuery(SearchQuery),         // /Patient/_search/gender=female
    SearchResultGroup(SearchResultGroup),
    OperationPath(OperationPath),           // /ViewDefinition/$run
    OperationExecution(OperationExecution), // /ViewDefinition/$run/view-def-1.json
}

#[derive(Debug)]
pub struct InodeIndex {
    entries: HashMap<u64, VFSEntry>,
    // Additional indexes for fast lookups
    resource_type_index: HashMap<String, Vec<u64>>, // resource_type -> [inodes]
    parent_child_index: HashMap<u64, Vec<u64>>,     // parent_inode -> [child_inodes]
}

impl InodeIndex {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            resource_type_index: HashMap::new(),
            parent_child_index: HashMap::new(),
        }
    }

    pub fn insert_directory(&mut self, directory: Directory) {
        let inode = directory.inode;
        self.entries.insert(inode, VFSEntry::Directory(directory));
    }

    pub fn insert_text_file(&mut self, text_file: TextFile) {
        let inode = text_file.inode;
        self.entries.insert(inode, VFSEntry::TextFile(text_file));
    }

    pub fn insert_search(&mut self, search: SearchPath) {
        let inode = search.inode;
        self.entries.insert(inode, VFSEntry::SearchPath(search));
    }

    pub fn insert_search_query(&mut self, query: SearchQuery) {
        let inode = query.inode;
        self.entries.insert(inode, VFSEntry::SearchQuery(query));
    }

    pub fn insert_search_result_group(&mut self, group: SearchResultGroup) {
        let inode = group.inode;
        self.entries
            .insert(inode, VFSEntry::SearchResultGroup(group));
    }

    pub fn insert_operation_path(&mut self, operation: OperationPath) {
        let inode = operation.inode;
        self.entries
            .insert(inode, VFSEntry::OperationPath(operation));
    }

    pub fn insert_operation_execution(&mut self, execution: OperationExecution) {
        let inode = execution.inode;
        self.entries
            .insert(inode, VFSEntry::OperationExecution(execution));
    }

    pub fn insert_resource(&mut self, resource: FHIRResource) {
        let inode = resource.inode;
        let resource_type = resource.resource_type.clone();

        // Update resource type index
        self.resource_type_index
            .entry(resource_type)
            .or_insert_with(Vec::new)
            .push(inode);

        self.entries.insert(inode, VFSEntry::FHIRResource(resource));
    }

    pub fn insert_resource_version(&mut self, version: ResourceVersion) {
        let inode = version.inode;
        self.entries
            .insert(inode, VFSEntry::ResourceVersion(version));
    }

    pub fn get(&self, inode: u64) -> Option<&VFSEntry> {
        self.entries.get(&inode)
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, inode: u64) -> Option<&mut VFSEntry> {
        self.entries.get_mut(&inode)
    }

    #[allow(dead_code)]
    pub fn contains(&self, inode: u64) -> bool {
        self.entries.contains_key(&inode)
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, inode: u64) -> Option<VFSEntry> {
        if let Some(entry) = self.entries.remove(&inode) {
            // Clean up indexes
            if let VFSEntry::FHIRResource(ref resource) = entry {
                if let Some(inodes) = self.resource_type_index.get_mut(&resource.resource_type) {
                    inodes.retain(|&i| i != inode);
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn get_resources_by_type(&self, resource_type: &str) -> Vec<&FHIRResource> {
        self.resource_type_index
            .get(resource_type)
            .map(|inodes| {
                inodes
                    .iter()
                    .filter_map(|&inode| match self.entries.get(&inode) {
                        Some(VFSEntry::FHIRResource(resource)) => Some(resource),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn clear_resources_by_type(&mut self, resource_type: &str) {
        if let Some(inodes) = self.resource_type_index.remove(resource_type) {
            // Create a set of inodes to remove for efficient lookup
            let inodes_set: std::collections::HashSet<u64> = inodes.iter().copied().collect();

            // Remove entries
            for inode in &inodes {
                self.entries.remove(inode);
            }

            // Remove from parent-child relations
            for children in self.parent_child_index.values_mut() {
                children.retain(|child| !inodes_set.contains(child));
            }
        }
    }

    pub fn add_parent_child_relation(&mut self, parent: u64, child: u64) {
        self.parent_child_index
            .entry(parent)
            .or_insert_with(Vec::new)
            .push(child);
    }

    pub fn get_children(&self, parent: u64) -> Vec<u64> {
        self.parent_child_index
            .get(&parent)
            .cloned()
            .unwrap_or_default()
    }

    pub fn clear_children(&mut self, parent: u64) {
        self.parent_child_index.remove(&parent);
    }

    pub fn get_attr(&self, inode: u64) -> Option<fuser::FileAttr> {
        self.entries.get(&inode).map(|entry| match entry {
            VFSEntry::Directory(dir) => dir.get_attr(),
            VFSEntry::TextFile(file) => file.get_attr(),
            VFSEntry::FHIRResource(resource) => resource.get_attr(),
            VFSEntry::ResourceVersion(version) => version.get_attr(),
            VFSEntry::SearchPath(search) => search.get_attr(),
            VFSEntry::SearchQuery(query) => query.get_attr(),
            VFSEntry::SearchResultGroup(group) => group.get_attr(),
            VFSEntry::OperationPath(op) => op.get_attr(),
            VFSEntry::OperationExecution(exec) => exec.get_attr(),
        })
    }

    pub fn find_child_by_name(&self, parent: u64, name: &str) -> Option<u64> {
        self.get_children(parent).into_iter().find(|&child_inode| {
            match self.entries.get(&child_inode) {
                Some(VFSEntry::Directory(dir)) => dir.name == name,
                Some(VFSEntry::TextFile(file)) => file.filename == name,
                Some(VFSEntry::FHIRResource(resource)) => resource.filename == name,
                Some(VFSEntry::ResourceVersion(version)) => version.filename == name,
                Some(VFSEntry::SearchPath(search)) => search.path == name,
                Some(VFSEntry::SearchQuery(query)) => query.query == name,
                Some(VFSEntry::SearchResultGroup(group)) => group.resource_type == name,
                Some(VFSEntry::OperationPath(op)) => op.path == name,
                Some(VFSEntry::OperationExecution(exec)) => exec.path == name,
                None => false,
            }
        })
    }

    pub fn get_search_path_info(&self, inode: u64) -> Option<(String, u64)> {
        if let Some(VFSEntry::SearchPath(search)) = self.get(inode) {
            Some((search.resource_type.clone(), inode))
        } else {
            None
        }
    }

    pub fn get_directory(&self, inode: u64) -> Option<&Directory> {
        if let Some(VFSEntry::Directory(directory)) = self.get(inode) {
            Some(directory)
        } else {
            None
        }
    }

    pub fn get_resource_version(&self, inode: u64) -> Option<&ResourceVersion> {
        if let Some(VFSEntry::ResourceVersion(version)) = self.get(inode) {
            Some(version)
        } else {
            None
        }
    }

    pub fn get_text_file(&self, inode: u64) -> Option<&TextFile> {
        if let Some(VFSEntry::TextFile(text_file)) = self.get(inode) {
            Some(text_file)
        } else {
            None
        }
    }

    pub fn get_fhir_resource(&self, inode: u64) -> Option<&FHIRResource> {
        if let Some(VFSEntry::FHIRResource(resource)) = self.get(inode) {
            Some(resource)
        } else {
            None
        }
    }

    pub fn get_search_path(&self, inode: u64) -> Option<&SearchPath> {
        if let Some(VFSEntry::SearchPath(search)) = self.get(inode) {
            Some(search)
        } else {
            None
        }
    }

    pub fn get_search_query(&self, inode: u64) -> Option<&SearchQuery> {
        if let Some(VFSEntry::SearchQuery(query)) = self.get(inode) {
            Some(query)
        } else {
            None
        }
    }

    pub fn get_search_result_group(&self, inode: u64) -> Option<&SearchResultGroup> {
        if let Some(VFSEntry::SearchResultGroup(group)) = self.get(inode) {
            Some(group)
        } else {
            None
        }
    }

    pub fn get_operation_path(&self, inode: u64) -> Option<&OperationPath> {
        if let Some(VFSEntry::OperationPath(op)) = self.get(inode) {
            Some(op)
        } else {
            None
        }
    }

    pub fn get_operation_execution(&self, inode: u64) -> Option<&OperationExecution> {
        if let Some(VFSEntry::OperationExecution(exec)) = self.get(inode) {
            Some(exec)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn iter_entries(&self) -> impl Iterator<Item = (&u64, &VFSEntry)> {
        self.entries.iter()
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> IndexStats {
        let mut stats = IndexStats::default();
        for entry in self.entries.values() {
            match entry {
                VFSEntry::Directory(_) => stats.directories += 1,
                VFSEntry::TextFile(_) => stats.text_files += 1,
                VFSEntry::FHIRResource(_) => stats.resources += 1,
                VFSEntry::ResourceVersion(_) => stats.resource_versions += 1,
                VFSEntry::SearchPath(_) => stats.search += 1,
                VFSEntry::SearchQuery(_) => stats.search_queries += 1,
                VFSEntry::SearchResultGroup(_) => stats.search_result_groups += 1,
                VFSEntry::OperationPath(_) => stats.operations += 1,
                VFSEntry::OperationExecution(_) => stats.operation_executions += 1,
            }
        }
        stats.total = self.entries.len();
        stats
    }
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct IndexStats {
    pub total: usize,
    pub directories: usize,
    pub text_files: usize,
    pub resources: usize,
    pub resource_versions: usize,
    pub search: usize,
    pub search_queries: usize,
    pub search_result_groups: usize,
    pub operations: usize,
    pub operation_executions: usize,
}

impl std::fmt::Display for IndexStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Total: {}, Directories: {}, Text Files: {}, Resources: {}, Versions: {}, Search: {}, Queries: {}, Groups: {}, Operations: {}, Executions: {}",
            self.total, self.directories, self.text_files, self.resources, self.resource_versions, self.search,
            self.search_queries, self.search_result_groups, self.operations, self.operation_executions
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_index_basic_operations() {
        let mut index = InodeIndex::new();

        // Test directory insertion
        let dir = Directory::new(1, "test_dir");
        index.insert_directory(dir.clone());

        assert!(index.contains(1));
        assert!(matches!(index.get(1), Some(VFSEntry::Directory(_))));

        // Test text file insertion
        let text_file = TextFile::new(2, "test.txt", "content");
        index.insert_text_file(text_file.clone());

        assert!(index.contains(2));
        assert!(matches!(index.get(2), Some(VFSEntry::TextFile(_))));

        // Test resource insertion
        let resource = FHIRResource::new(3, "Patient", "123", "{}");
        index.insert_resource(resource.clone());

        assert!(index.contains(3));
        assert!(matches!(index.get(3), Some(VFSEntry::FHIRResource(_))));

        // Test stats
        let stats = index.stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.directories, 1);
        assert_eq!(stats.text_files, 1);
        assert_eq!(stats.resources, 1);
    }

    #[test]
    fn test_resource_type_index() {
        let mut index = InodeIndex::new();

        let resource1 = FHIRResource::new(1, "Patient", "123", "{}");
        let resource2 = FHIRResource::new(2, "Patient", "456", "{}");
        let resource3 = FHIRResource::new(3, "Observation", "789", "{}");

        index.insert_resource(resource1);
        index.insert_resource(resource2);
        index.insert_resource(resource3);

        let patients = index.get_resources_by_type("Patient");
        assert_eq!(patients.len(), 2);

        let observations = index.get_resources_by_type("Observation");
        assert_eq!(observations.len(), 1);

        index.clear_resources_by_type("Patient");
        let patients_after = index.get_resources_by_type("Patient");
        assert_eq!(patients_after.len(), 0);

        // Observation should still exist
        assert!(index.contains(3));
    }

    #[test]
    fn test_parent_child_relations() {
        let mut index = InodeIndex::new();

        index.add_parent_child_relation(1, 2);
        index.add_parent_child_relation(1, 3);
        index.add_parent_child_relation(2, 4);

        let children_of_1 = index.get_children(1);
        assert_eq!(children_of_1, vec![2, 3]);

        let children_of_2 = index.get_children(2);
        assert_eq!(children_of_2, vec![4]);

        let children_of_3 = index.get_children(3);
        assert_eq!(children_of_3, Vec::<u64>::new());
    }

    #[test]
    fn test_find_child_by_name() {
        let mut index = InodeIndex::new();

        let dir = Directory::new(1, "parent");
        let child_dir = Directory::new(2, "child_dir");
        let child_file = TextFile::new(3, "child.txt", "content");

        index.insert_directory(dir);
        index.insert_directory(child_dir);
        index.insert_text_file(child_file);

        index.add_parent_child_relation(1, 2);
        index.add_parent_child_relation(1, 3);

        assert_eq!(index.find_child_by_name(1, "child_dir"), Some(2));
        assert_eq!(index.find_child_by_name(1, "child.txt"), Some(3));
        assert_eq!(index.find_child_by_name(1, "nonexistent"), None);
    }
}
