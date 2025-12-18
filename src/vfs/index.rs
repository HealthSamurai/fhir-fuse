use super::directory::Directory;
use super::resource::FHIRResource;
use super::text_file::TextFile;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum VFSEntry {
    Directory(Directory),
    TextFile(TextFile),
    FHIRResource(FHIRResource),
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
            for inode in inodes {
                self.entries.remove(&inode);
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

    pub fn get_attr(&self, inode: u64) -> Option<fuser::FileAttr> {
        self.entries.get(&inode).map(|entry| match entry {
            VFSEntry::Directory(dir) => dir.get_attr(),
            VFSEntry::TextFile(file) => file.get_attr(),
            VFSEntry::FHIRResource(resource) => resource.get_attr(),
        })
    }

    pub fn find_child_by_name(&self, parent: u64, name: &str) -> Option<u64> {
        self.get_children(parent).into_iter().find(|&child_inode| {
            match self.entries.get(&child_inode) {
                Some(VFSEntry::Directory(dir)) => dir.name == name,
                Some(VFSEntry::TextFile(file)) => file.filename == name,
                Some(VFSEntry::FHIRResource(resource)) => resource.filename == name,
                None => false,
            }
        })
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
}

impl std::fmt::Display for IndexStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Total: {}, Directories: {}, Text Files: {}, Resources: {}",
            self.total, self.directories, self.text_files, self.resources
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
