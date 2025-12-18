#[derive(Debug)]
pub struct InodeAllocator {
    next_inode: u64,
}

impl InodeAllocator {
    pub fn new(start_inode: u64) -> Self {
        Self {
            next_inode: start_inode,
        }
    }

    pub fn allocate(&mut self) -> u64 {
        let inode = self.next_inode;
        self.next_inode += 1;
        inode
    }

    pub fn allocate_range(&mut self, count: usize) -> Vec<u64> {
        let mut allocated = Vec::with_capacity(count);
        for _ in 0..count {
            allocated.push(self.allocate());
        }
        allocated
    }

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
        assert_eq!(allocator.allocate(), 1);
        assert_eq!(allocator.allocate(), 2);
        assert_eq!(allocator.allocate(), 3);
    }

    #[test]
    fn test_allocate_range() {
        let mut allocator = InodeAllocator::new(100);
        let range = allocator.allocate_range(5);
        assert_eq!(range, vec![100, 101, 102, 103, 104]);
        assert_eq!(allocator.allocate(), 105);
    }

    #[test]
    fn test_peek_next() {
        let mut allocator = InodeAllocator::new(1);
        assert_eq!(allocator.peek_next(), 1);

        allocator.allocate();
        assert_eq!(allocator.peek_next(), 2);

        allocator.allocate();
        allocator.allocate();
        assert_eq!(allocator.peek_next(), 4);
    }
}
