//! The seam between `WakeLock` and a platform: query it, take a lock, give it back.

use std::{future::Future, pin::Pin};

use crate::{error::WakeLockError, options::WakeLockOptions};

/// A future a backend returns. Each call's future borrows the backend.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// A lock a backend issued, opaque to everyone but the backend that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockId(u64);

impl LockId {
    /// Issues id number `value`. Only backends do this.
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }
}

/// What a platform provides. [`crate::WakeLock`] adds the rules: the options must request a lock,
/// and a guard releases exactly once.
///
/// `release` is synchronous because every platform's release call is sync-friendly, and the
/// guard's `Drop` needs it.
pub trait Backend: Send + Sync {
    /// Whether the platform or browser can hold a wake lock here.
    fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>>;

    /// Takes the lock the options request and returns its id.
    fn acquire(
        &self,
        options: &WakeLockOptions,
    ) -> BackendFuture<'_, Result<LockId, WakeLockError>>;

    /// Gives the lock back. Releasing an id this backend never issued is `NotAcquired`.
    fn release(&self, id: LockId) -> Result<(), WakeLockError>;

    /// Whether a lock this backend issued is still held.
    fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>>;
}
