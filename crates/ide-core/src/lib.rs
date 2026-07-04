use std::fmt;

#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("GPU/rendering error: {0}")]
    Gpu(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(u64);

impl EntityId {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Default)]
pub struct EntityIdAllocator {
    next: std::sync::atomic::AtomicU64,
}

impl EntityIdAllocator {
    pub fn new() -> Self {
        Self {
            next: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn allocate(&self) -> EntityId {
        let raw = self
            .next
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        EntityId::new(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ids_are_unique_and_increasing() {
        let allocator = EntityIdAllocator::new();
        let a = allocator.allocate();
        let b = allocator.allocate();
        assert_ne!(a, b);
        assert!(b.raw() > a.raw());
    }
}