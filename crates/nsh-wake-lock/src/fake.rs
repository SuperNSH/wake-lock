//! A backend with scripted answers. Tests use it, and so can an app without a wake-lock service.

use std::{
    future::ready,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    backend::{Backend, BackendFuture, LockId},
    error::{ErrorKind, WakeLockError},
    options::WakeLockOptions,
};

/// A backend whose answers the test sets, and whose state the test reads back. Clones share one
/// state.
#[derive(Clone)]
pub struct FakeBackend {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    supported: Result<bool, WakeLockError>,
    acquire_answer: Result<(), WakeLockError>,
    next_release_error: Option<WakeLockError>,
    next_id: u64,
    active: Vec<LockId>,
    acquires: usize,
    releases: usize,
    last_options: Option<WakeLockOptions>,
}

impl Default for FakeBackend {
    /// A supported platform that hands out locks.
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    /// A supported platform whose acquires succeed with a fresh id.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                supported: Ok(true),
                acquire_answer: Ok(()),
                next_release_error: None,
                next_id: 0,
                active: Vec::new(),
                acquires: 0,
                releases: 0,
                last_options: None,
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets what `is_supported` answers.
    pub fn set_supported(&self, answer: Result<bool, WakeLockError>) {
        self.lock().supported = answer;
    }

    /// Sets what every `acquire` answers: `Ok(())` issues a fresh id, an `Err` fails each call
    /// until the answer changes.
    pub fn set_acquire_answer(&self, answer: Result<(), WakeLockError>) {
        self.lock().acquire_answer = answer;
    }

    /// Makes the next `release` of a held lock fail with `error`. `None` lets releases succeed
    /// again.
    pub fn fail_next_release(&self, error: Option<WakeLockError>) {
        self.lock().next_release_error = error;
    }

    /// How many locks were acquired.
    pub fn acquires(&self) -> usize {
        self.lock().acquires
    }

    /// How many locks were released.
    pub fn releases(&self) -> usize {
        self.lock().releases
    }

    /// The ids of locks issued and not yet released.
    pub fn active_ids(&self) -> Vec<LockId> {
        self.lock().active.clone()
    }

    /// The options the last `acquire` was called with.
    pub fn last_options(&self) -> Option<WakeLockOptions> {
        self.lock().last_options.clone()
    }
}

impl Backend for FakeBackend {
    fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        let answer = self.lock().supported.clone();
        Box::pin(ready(answer))
    }

    fn acquire(
        &self,
        options: &WakeLockOptions,
    ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
        let answer = {
            let mut state = self.lock();
            state.last_options = Some(options.clone());
            match state.acquire_answer.clone() {
                Ok(()) => {
                    let id = LockId::new(state.next_id);
                    state.next_id += 1;
                    state.active.push(id);
                    state.acquires += 1;
                    Ok(id)
                }
                Err(error) => Err(error),
            }
        };
        Box::pin(ready(answer))
    }

    fn release(&self, id: LockId) -> Result<(), WakeLockError> {
        let mut state = self.lock();
        if !state.active.contains(&id) {
            return Err(WakeLockError::new(ErrorKind::NotAcquired));
        }
        if let Some(error) = state.next_release_error.take() {
            return Err(error);
        }
        state.active.retain(|&active| active != id);
        state.releases += 1;
        Ok(())
    }

    fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        let active = !self.lock().active.is_empty();
        Box::pin(ready(Ok::<bool, WakeLockError>(active)))
    }
}
