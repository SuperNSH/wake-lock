//! The brief's platform edge cases, scripted through the fake backend.

use nsh_wake_lock::{Backend, ErrorKind, FakeBackend, WakeLock, WakeLockError, WakeLockOptions};

// WakeLockOptions is non_exhaustive, so outside the crate it only grows from Default.
fn display_lock() -> WakeLockOptions {
    let mut options = WakeLockOptions::default();
    options.display = true;
    options
}

#[tokio::test]
async fn unsupported_platform_reports_not_supported() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    backend.set_supported(Ok(false));
    assert_eq!(
        service.is_supported().await,
        Ok(false),
        "a platform without wake locks answers false, not an error"
    );

    backend.set_acquire_answer(Err(WakeLockError::new(ErrorKind::NotSupported)));
    let Err(error) = service.acquire(&display_lock()).await else {
        panic!("acquire without platform support must fail");
    };
    assert_eq!(error.kind(), ErrorKind::NotSupported);
    assert_eq!(error.code(), "ERR_WAKE_LOCK_NOT_SUPPORTED");
    assert_eq!(backend.acquires(), 0, "no lock was issued");
    assert_eq!(service.is_active().await, Ok(false), "no lock is held");
}

#[tokio::test]
async fn backgrounded_reports_backgrounded() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    backend.set_acquire_answer(Err(WakeLockError::new(ErrorKind::Backgrounded)));
    let Err(error) = service.acquire(&display_lock()).await else {
        panic!("acquire while the app is backgrounded must fail");
    };
    assert_eq!(error.kind(), ErrorKind::Backgrounded);
    assert_eq!(error.code(), "ERR_WAKE_LOCK_BACKGROUNDED");
    assert_eq!(backend.acquires(), 0, "no lock was issued");
    assert_eq!(service.is_active().await, Ok(false), "no lock is held");
}

#[tokio::test]
async fn missing_activity_reports_no_activity() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    backend.set_acquire_answer(Err(WakeLockError::new(ErrorKind::NoActivity)));
    let Err(error) = service.acquire(&display_lock()).await else {
        panic!("acquire without a window or activity must fail");
    };
    assert_eq!(error.kind(), ErrorKind::NoActivity);
    assert_eq!(error.code(), "ERR_WAKE_LOCK_NO_ACTIVITY");
    assert_eq!(backend.acquires(), 0, "no lock was issued");
    assert_eq!(service.is_active().await, Ok(false), "no lock is held");
}

#[tokio::test]
async fn unknown_handle_release_reports_not_acquired() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    let Ok(guard) = service.acquire(&display_lock()).await else {
        panic!("acquire on a supported fake backend must succeed");
    };
    let ids = backend.active_ids();
    assert_eq!(ids.len(), 1, "exactly one lock was issued");
    let id = ids[0];

    // Release the id behind the guard's back, so the guard's handle goes stale.
    assert_eq!(
        backend.release(id),
        Ok(()),
        "the stolen release of a held lock succeeds"
    );

    let Err(error) = guard.release() else {
        panic!("releasing a stale handle must report not acquired");
    };
    assert_eq!(error.kind(), ErrorKind::NotAcquired);
    assert_eq!(error.code(), "ERR_WAKE_LOCK_NOT_ACQUIRED");

    let Err(again) = backend.release(id) else {
        panic!("re-releasing a released id must report not acquired");
    };
    assert_eq!(again.code(), "ERR_WAKE_LOCK_NOT_ACQUIRED");
    assert_eq!(backend.releases(), 1, "only the successful release counted");
}
