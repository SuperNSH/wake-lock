//! Wake locks for Native Shell apps: keep the screen or the machine awake while the app needs it.
//!
//! [`WakeLock`] takes a lock through the platform's backend and hands back a [`WakeLockGuard`].
//! The guard releases the lock exactly once: [`WakeLockGuard::release`] reports what happened, a
//! drop releases silently. Every call resolves exactly once: `Ok`, or a [`WakeLockError`] with a
//! stable code.

mod backend;
mod error;
mod fake;
mod lock;
mod options;
mod platform;

pub use backend::{Backend, BackendFuture, LockId};
pub use error::{ErrorKind, WakeLockError};
pub use fake::FakeBackend;
pub use lock::{WakeLock, WakeLockGuard};
pub use options::WakeLockOptions;
