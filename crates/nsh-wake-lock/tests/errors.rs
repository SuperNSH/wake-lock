//! The error model: every kind keeps its stable code and message, and failures carry what they should.

use nsh_wake_lock::{ErrorKind, FakeBackend, WakeLock, WakeLockError, WakeLockOptions};

// WakeLockOptions is non_exhaustive, so outside the crate it only grows from Default.
fn display_lock() -> WakeLockOptions {
    let mut options = WakeLockOptions::default();
    options.display = true;
    options
}

#[test]
fn every_kind_carries_its_stable_code() {
    let table = [
        (
            ErrorKind::NotSupported,
            "ERR_WAKE_LOCK_NOT_SUPPORTED",
            "Wake locks are not supported on this platform or browser.",
        ),
        (
            ErrorKind::InvalidArgument,
            "ERR_WAKE_LOCK_INVALID_ARGUMENT",
            "The options do not request any lock: set display, idle or sleep.",
        ),
        (
            ErrorKind::NoActivity,
            "ERR_WAKE_LOCK_NO_ACTIVITY",
            "There is no window or activity to hold the lock on right now.",
        ),
        (
            ErrorKind::Backgrounded,
            "ERR_WAKE_LOCK_BACKGROUNDED",
            "The app is backgrounded, so the idle-timer change does not apply now.",
        ),
        (
            ErrorKind::Timeout,
            "ERR_WAKE_LOCK_TIMEOUT",
            "The platform took too long to answer. Try again.",
        ),
        (
            ErrorKind::NotAcquired,
            "ERR_WAKE_LOCK_NOT_ACQUIRED",
            "That lock is not held: it was released already or never acquired.",
        ),
        (
            ErrorKind::Backend,
            "ERR_WAKE_LOCK_BACKEND",
            "The platform's wake lock service failed.",
        ),
    ];

    for (kind, code, message) in table {
        let error = WakeLockError::new(kind);
        assert_eq!(error.kind(), kind, "{kind:?} keeps its kind");
        assert_eq!(error.code(), code, "{kind:?} keeps its stable code");
        assert_eq!(kind.message(), message, "{kind:?} keeps its message");
        assert_eq!(
            error.to_string(),
            message,
            "{kind:?} shows its message through Display"
        );
        assert_eq!(
            error.status(),
            None,
            "{kind:?} carries no status unless the platform gives one"
        );
    }
}

#[tokio::test]
async fn all_false_options_are_invalid_argument() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    let Err(error) = service.acquire(&WakeLockOptions::default()).await else {
        panic!("options that request no lock must be refused");
    };
    assert_eq!(
        error.kind(),
        ErrorKind::InvalidArgument,
        "all-false options are an invalid argument"
    );
    assert_eq!(error.code(), "ERR_WAKE_LOCK_INVALID_ARGUMENT");
    assert_eq!(
        backend.acquires(),
        0,
        "an invalid request never reaches the backend"
    );
    assert_eq!(backend.last_options(), None, "the backend saw no options");
}

#[tokio::test]
async fn scripted_backend_error_surfaces_with_code() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());
    backend.set_acquire_answer(Err(WakeLockError::new(ErrorKind::Backend).with_status(4298)));

    let Err(error) = service.acquire(&display_lock()).await else {
        panic!("a failing backend must fail the acquire");
    };
    assert_eq!(
        error.kind(),
        ErrorKind::Backend,
        "the backend's kind surfaces"
    );
    assert_eq!(error.code(), "ERR_WAKE_LOCK_BACKEND");
    assert_eq!(
        error.status(),
        Some(4298),
        "the platform's own status code rides along"
    );
    assert_eq!(backend.acquires(), 0, "no lock was issued");
}
