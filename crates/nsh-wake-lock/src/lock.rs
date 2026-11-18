//! `WakeLock`: the calls an app makes. A guard holds one lock and releases it exactly once.

use std::sync::{Arc, OnceLock};

use crate::{
    backend::{Backend, LockId},
    error::{ErrorKind, WakeLockError},
    options::WakeLockOptions,
};

/// The platform's wake-lock service, as `wakelock_plus` and `expo-keep-awake` expose one.
///
/// [`WakeLock::new`] returns the process's shared service, so every caller shares one backend.
/// Any number of guards may be held at once; there is no single-flight gate.
#[derive(Clone)]
pub struct WakeLock {
    backend: Arc<dyn Backend>,
}

impl WakeLock {
    /// The platform's wake-lock service, one backend for the whole process.
    pub fn new() -> Self {
        static PLATFORM: OnceLock<WakeLock> = OnceLock::new();
        PLATFORM
            .get_or_init(|| Self {
                backend: crate::platform::default_backend(),
            })
            .clone()
    }

    /// A wake-lock service through `backend`. Tests and fakes use this.
    pub fn with_backend(backend: impl Backend + 'static) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Whether the platform or browser can hold a wake lock here.
    pub async fn is_supported(&self) -> Result<bool, WakeLockError> {
        self.backend.is_supported().await
    }

    /// Takes the lock the options request and returns a guard that holds it.
    ///
    /// Options with `display`, `idle` and `sleep` all false are `InvalidArgument`. The guard
    /// releases the lock exactly once: [`WakeLockGuard::release`] reports what happened, and a
    /// drop releases silently.
    pub async fn acquire(&self, options: &WakeLockOptions) -> Result<WakeLockGuard, WakeLockError> {
        if !options.display && !options.idle && !options.sleep {
            return Err(WakeLockError::new(ErrorKind::InvalidArgument));
        }
        let id = self.backend.acquire(options).await?;
        Ok(WakeLockGuard {
            backend: Arc::clone(&self.backend),
            id: Some(id),
        })
    }

    /// Whether a lock this service issued is still held, as `wakelock_plus` reports `enabled`.
    pub async fn is_active(&self) -> Result<bool, WakeLockError> {
        self.backend.is_active().await
    }
}

impl Default for WakeLock {
    fn default() -> Self {
        Self::new()
    }
}

/// One held lock, released exactly once: by [`WakeLockGuard::release`](Self::release), or by drop.
pub struct WakeLockGuard {
    backend: Arc<dyn Backend>,
    id: Option<LockId>,
}

impl WakeLockGuard {
    /// Releases the lock now and reports what the platform said. The guard disarms, so the drop
    /// that follows releases nothing.
    pub fn release(mut self) -> Result<(), WakeLockError> {
        let Some(id) = self.id.take() else {
            return Ok(());
        };
        self.backend.release(id)
    }
}

impl Drop for WakeLockGuard {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            // Release on drop must never panic: the guard is going away, so a failed release is
            // dropped on the floor.
            let _ = self.backend.release(id);
        }
    }
}
