//! The guard's lifecycle: a held lock releases exactly once, by drop, by unwind, or by call.

use std::panic::{AssertUnwindSafe, catch_unwind};

use nsh_wake_lock::{Backend, ErrorKind, FakeBackend, WakeLock, WakeLockError, WakeLockOptions};

// WakeLockOptions is non_exhaustive, so outside the crate it only grows from Default.
fn display_lock() -> WakeLockOptions {
    let mut options = WakeLockOptions::default();
    options.display = true;
    options
}

#[tokio::test]
async fn drop_releases_the_lock() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    {
        let Ok(_guard) = service.acquire(&display_lock()).await else {
            panic!("acquire on a supported fake backend must succeed");
        };
        assert_eq!(
            backend.releases(),
            0,
            "nothing is released while the guard lives"
        );
    }

    assert_eq!(backend.acquires(), 1, "one lock was acquired");
    assert_eq!(
        backend.releases(),
        1,
        "the dropped guard released exactly once"
    );
    assert!(
        backend.active_ids().is_empty(),
        "no lock is still held after the drop"
    );
}

#[tokio::test]
async fn unwind_releases_the_lock() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    let Ok(guard) = service.acquire(&display_lock()).await else {
        panic!("acquire on a supported fake backend must succeed");
    };

    // The guard holds Arc<dyn Backend>, which is not UnwindSafe, so the scope must be asserted.
    let unwind = catch_unwind(AssertUnwindSafe(move || {
        let _held = guard;
        panic!("the panic this scope unwinds through");
    }));

    assert!(unwind.is_err(), "the scope must have panicked");
    assert_eq!(
        backend.releases(),
        1,
        "the unwinding drop released exactly once"
    );
    assert!(
        backend.active_ids().is_empty(),
        "no lock is still held after the unwind"
    );
}

#[tokio::test]
async fn explicit_release_releases_exactly_once() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    let Ok(guard) = service.acquire(&display_lock()).await else {
        panic!("acquire on a supported fake backend must succeed");
    };

    assert_eq!(guard.release(), Ok(()), "the explicit release succeeds");
    assert_eq!(
        backend.releases(),
        1,
        "the release and the drop that consumes the guard touch the backend once"
    );
    assert!(
        backend.active_ids().is_empty(),
        "no lock is still held after the release"
    );
}

#[tokio::test]
async fn failed_release_on_drop_does_not_panic() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    backend.fail_next_release(Some(WakeLockError::new(ErrorKind::Backend)));

    {
        let Ok(_guard) = service.acquire(&display_lock()).await else {
            panic!("acquire on a supported fake backend must succeed");
        };
    }

    // Reaching here is the assertion: the drop swallowed the failure without panicking.
    assert_eq!(
        backend.releases(),
        0,
        "a failed release does not count as released"
    );
    let ids = backend.active_ids();
    assert_eq!(ids.len(), 1, "the failed release left the lock held");

    // The drop's attempt consumed the one-shot failure, so the same id now releases.
    assert_eq!(
        backend.release(ids[0]),
        Ok(()),
        "the drop's release attempt reached the backend"
    );
    assert_eq!(backend.releases(), 1, "one release has now succeeded");
}

#[tokio::test]
async fn multiple_locks_coexist_until_each_releases() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    let Ok(first) = service.acquire(&display_lock()).await else {
        panic!("the first acquire on a supported fake backend must succeed");
    };
    let sleep_lock = {
        let mut options = WakeLockOptions::default();
        options.sleep = true;
        options
    };
    let Ok(second) = service.acquire(&sleep_lock).await else {
        panic!("the second acquire on a supported fake backend must succeed");
    };

    assert_eq!(backend.acquires(), 2, "two locks were issued");
    assert_eq!(service.is_active().await, Ok(true), "a lock is held");

    assert_eq!(
        first.release(),
        Ok(()),
        "the first guard releases its own lock"
    );
    assert_eq!(
        service.is_active().await,
        Ok(true),
        "the second lock is still held after the first releases"
    );

    assert_eq!(
        second.release(),
        Ok(()),
        "the second guard releases its own lock"
    );
    assert_eq!(
        service.is_active().await,
        Ok(false),
        "no lock is held once both guards have released"
    );
    assert!(
        backend.active_ids().is_empty(),
        "the backend holds nothing after both releases"
    );
}

#[tokio::test]
async fn is_active_reports_whether_a_lock_is_held() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    assert_eq!(
        service.is_active().await,
        Ok(false),
        "no lock is held before any acquire"
    );

    let Ok(guard) = service.acquire(&display_lock()).await else {
        panic!("acquire on a supported fake backend must succeed");
    };
    assert_eq!(
        service.is_active().await,
        Ok(true),
        "a held guard keeps the lock active"
    );

    assert_eq!(guard.release(), Ok(()), "the explicit release succeeds");
    assert_eq!(
        service.is_active().await,
        Ok(false),
        "no lock is held after the release"
    );
}

#[tokio::test]
async fn options_reach_the_backend() {
    let backend = FakeBackend::new();
    let service = WakeLock::with_backend(backend.clone());

    let mut options = WakeLockOptions::default();
    options.display = true;
    options.sleep = true;
    options.reason = Some("Encoding the last chapter".into());
    options.app_name = Some("NSH Encoder".into());
    options.app_reverse_domain = Some("io.github.supernsh.encoder".into());

    let Ok(_guard) = service.acquire(&options).await else {
        panic!("acquire on a supported fake backend must succeed");
    };

    assert_eq!(
        backend.last_options(),
        Some(options),
        "the backend sees the flags and the names unchanged"
    );
}
