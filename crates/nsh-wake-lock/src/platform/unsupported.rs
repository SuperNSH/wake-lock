//! A target with no bridge: wake locks do not exist here, and this backend fakes no success.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{Backend, BackendFuture, LockId},
    error::{ErrorKind, WakeLockError},
    options::WakeLockOptions,
};

/// The backend of a target this crate has no bridge for.
pub(crate) fn backend() -> Arc<dyn Backend> {
    Arc::new(UnsupportedBackend)
}

struct UnsupportedBackend;

impl Backend for UnsupportedBackend {
    fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        Box::pin(ready(Ok::<bool, WakeLockError>(false)))
    }

    fn acquire(
        &self,
        _options: &WakeLockOptions,
    ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
        Box::pin(ready(Err(WakeLockError::new(ErrorKind::NotSupported))))
    }

    fn release(&self, _id: LockId) -> Result<(), WakeLockError> {
        Ok(())
    }

    fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        Box::pin(ready(Ok::<bool, WakeLockError>(false)))
    }
}
