#[derive(Debug)]
pub struct InodeAllocator {
    pub root_inode: u64,
    next_inode: u64,
}

impl InodeAllocator {
    pub fn new(start_inode: u64) -> Self {
        Self {
            root_inode: start_inode,
            next_inode: start_inode + 1,
        }
    }

    pub fn allocate(&mut self) -> u64 {
        let inode = self.next_inode;
        self.next_inode += 1;
        inode
    }

    #[allow(dead_code)]
    pub fn allocate_range(&mut self, count: usize) -> Vec<u64> {
        let mut allocated = Vec::with_capacity(count);
        for _ in 0..count {
            allocated.push(self.allocate());
        }
        allocated
    }

    #[allow(dead_code)]
    pub fn peek_next(&self) -> u64 {
        self.next_inode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let mut allocator = InodeAllocator::new(1);
        assert_eq!(allocator.allocate(), 2);
        assert_eq!(allocator.allocate(), 3);
        assert_eq!(allocator.allocate(), 4);
    }

    #[test]
    fn test_allocate_range() {
        let mut allocator = InodeAllocator::new(100);
        let range = allocator.allocate_range(5);
        assert_eq!(range, vec![101, 102, 103, 104, 105]);
        assert_eq!(allocator.allocate(), 106);
    }

    #[test]
    fn test_peek_next() {
        let mut allocator = InodeAllocator::new(1);
        assert_eq!(allocator.peek_next(), 2);

        allocator.allocate();
        assert_eq!(allocator.peek_next(), 3);

        allocator.allocate();
        allocator.allocate();
        assert_eq!(allocator.peek_next(), 5);
    }
}
