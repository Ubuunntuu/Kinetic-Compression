use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use crate::EngineError;

pub struct MemoryArena {
    capacity_bytes: AtomicU64,
    allocated_bytes: AtomicU64,
    peak_bytes: AtomicU64,
    wait_lock: Mutex<()>,
    wait_cv: Condvar,
}

impl MemoryArena {
    pub fn new(capacity_mb: u64) -> Arc<Self> {
        let cap = (capacity_mb.max(64)) * 1024 * 1024;
        Arc::new(Self {
            capacity_bytes: AtomicU64::new(cap),
            allocated_bytes: AtomicU64::new(0),
            peak_bytes: AtomicU64::new(0),
            wait_lock: Mutex::new(()),
            wait_cv: Condvar::new(),
        })
    }

    pub fn set_capacity_mb(&self, capacity_mb: u64) {
        let cap = (capacity_mb.max(64)) * 1024 * 1024;
        self.capacity_bytes.store(cap, Ordering::SeqCst);
        self.wait_cv.notify_all();
    }

    pub fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes.load(Ordering::Relaxed)
    }

    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    pub fn peak_bytes(&self) -> u64 {
        self.peak_bytes.load(Ordering::Relaxed)
    }

    pub fn allocate(self: &Arc<Self>, size: usize) -> Result<ArenaBuffer, EngineError> {
        let req = size as u64;
        let cap = self.capacity_bytes();
        if req > cap {
            return Err(EngineError::InvalidConfig(format!(
                "Requested block buffer ({} MB) exceeds total memory arena cap ({} MB)",
                req / (1024 * 1024),
                cap / (1024 * 1024)
            )));
        }

        let mut guard = self.wait_lock.lock().unwrap();
        loop {
            let current = self.allocated_bytes.load(Ordering::SeqCst);
            if current + req <= self.capacity_bytes.load(Ordering::SeqCst) {
                let next = current + req;
                if self
                    .allocated_bytes
                    .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    self.peak_bytes.fetch_max(next, Ordering::SeqCst);
                    break;
                }
            } else {
                guard = self.wait_cv.wait(guard).unwrap();
            }
        }
        drop(guard);

        Ok(ArenaBuffer {
            arena: Arc::clone(self),
            data: vec![0u8; size],
            reserved_bytes: req,
        })
    }

    fn release(&self, bytes: u64) {
        self.allocated_bytes.fetch_sub(bytes, Ordering::SeqCst);
        self.wait_cv.notify_all();
    }
}

pub struct ArenaBuffer {
    arena: Arc<MemoryArena>,
    pub data: Vec<u8>,
    reserved_bytes: u64,
}

impl Drop for ArenaBuffer {
    fn drop(&mut self) {
        self.arena.release(self.reserved_bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_arena_allocation_and_raii_release() {
        let arena = MemoryArena::new(64);
        assert_eq!(arena.allocated_bytes(), 0);

        {
            let buf1 = arena.allocate(4 * 1024 * 1024).unwrap();
            assert_eq!(buf1.data.len(), 4 * 1024 * 1024);
            assert_eq!(arena.allocated_bytes(), 4 * 1024 * 1024);

            let _buf2 = arena.allocate(8 * 1024 * 1024).unwrap();
            assert_eq!(arena.allocated_bytes(), 12 * 1024 * 1024);
            assert_eq!(arena.peak_bytes(), 12 * 1024 * 1024);
        }

        assert_eq!(arena.allocated_bytes(), 0);
        assert_eq!(arena.peak_bytes(), 12 * 1024 * 1024);
    }
}