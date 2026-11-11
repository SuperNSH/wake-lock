//! The Linux bridge: `org.freedesktop.ScreenSaver` on the session bus keeps the display on, and
//! systemd's `org.freedesktop.login1.Manager` on the system bus holds the idle and sleep
//! inhibitor locks. Debug with `systemd-inhibit --list`.

use std::{
    collections::HashMap,
    future::{Future, poll_fn, ready},
    pin::pin,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    task::{Context, Poll, Waker},
    thread,
    time::{Duration, Instant},
};

use zbus::{Connection, OwnedFd, proxy};

use crate::{
    backend::{Backend, BackendFuture, LockId},
    error::{ErrorKind, WakeLockError},
    options::WakeLockOptions,
};

// The defaults options.rs documents, spelled once for the name each D-Bus call asks with.
const DEFAULT_REASON: &str = "User requested";
const DEFAULT_APP_NAME: &str = "nsh-wake-lock";
const DEFAULT_APP_REVERSE_DOMAIN: &str = "io.github.supernsh.wake-lock";

/// One D-Bus call may take at most this long before it fails with `Timeout`. One acquire
/// runs up to eight calls in sequence (about 40 s at the worst), so this is not its bound.
const CALL_BOUND: Duration = Duration::from_secs(5);

/// The release wait holds out for the call bound plus this much thread-scheduling headroom.
const JOIN_BOUND: Duration = Duration::from_secs(6);

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    /// Takes an inhibitor lock, which lasts exactly as long as the returned fd stays open.
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;
}

#[proxy(assume_defaults = true)]
trait ScreenSaver {
    /// Inhibits the display from blanking; the returned cookie ends the inhibit.
    fn inhibit(&self, application_name: &str, reason_for_inhibit: &str) -> zbus::Result<u32>;

    /// Ends the inhibit of `cookie`.
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}

/// The backend of a Linux desktop or server: one inhibit per requested flag, tracked under a
/// fresh id per acquire.
struct LinuxBackend {
    locks: Mutex<HashMap<LockId, HeldLock>>,
    next_id: AtomicU64,
}

/// The backend of the Linux target.
pub(crate) fn backend() -> Arc<dyn Backend> {
    Arc::new(LinuxBackend {
        locks: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(0),
    })
}

/// What one acquire holds: the screensaver cookie, and the logind fds that are the locks.
struct HeldLock {
    display: Option<(ScreenSaverProxy<'static>, u32)>,
    idle_fd: Option<OwnedFd>,
    sleep_fd: Option<OwnedFd>,
}

impl HeldLock {
    /// Gives every inhibit back: the fds close, the cookie is un-inhibited.
    async fn release(self) -> Result<(), WakeLockError> {
        // A logind inhibitor lock lasts exactly as long as its fd: dropping it releases.
        let HeldLock {
            display,
            idle_fd: _,
            sleep_fd: _,
        } = self;
        match display {
            None => Ok(()),
            Some((screensaver, cookie)) => bounded(screensaver.un_inhibit(cookie)).await,
        }
    }
}

/// Races one D-Bus call against `CALL_BOUND`, leaving the call's own answer untouched.
async fn raced<F, T>(call: F) -> Result<Result<T, zbus::Error>, WakeLockError>
where
    F: Future<Output = zbus::Result<T>>,
{
    let deadline = Deadline::start(CALL_BOUND);
    let mut call = pin!(call);
    poll_fn(|cx| match call.as_mut().poll(cx) {
        Poll::Ready(answer) => {
            deadline.finish();
            Poll::Ready(Ok(answer))
        }
        Poll::Pending if deadline.expired(cx) => {
            Poll::Ready(Err(WakeLockError::new(ErrorKind::Timeout)))
        }
        Poll::Pending => Poll::Pending,
    })
    .await
}

/// Bounds one D-Bus call at `CALL_BOUND`: a missed deadline is `Timeout`, a bus error is
/// `Backend`.
async fn bounded<F, T>(call: F) -> Result<T, WakeLockError>
where
    F: Future<Output = zbus::Result<T>>,
{
    raced(call)
        .await
        .and_then(|answer| answer.map_err(backend_error))
}

/// D-Bus reports error names, not numeric statuses, so a Linux failure carries no status.
fn backend_error(_error: zbus::Error) -> WakeLockError {
    WakeLockError::new(ErrorKind::Backend)
}

/// Whether the error settles that no bus is there to talk to at all.
fn no_bus_at_all(error: &zbus::Error) -> bool {
    // Only a missing or refused bus socket says "no D-Bus here"; anything else — a failed
    // handshake, permissions, a bad address — is a failure to report, not a missing service.
    let zbus::Error::InputOutput(io) = error else {
        return false;
    };
    matches!(
        io.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
    )
}

/// A deadline raced against a D-Bus call. A plain thread is the timer: it works under every
/// executor a caller may poll this future with, and this crate carries no timer of its own.
struct Deadline {
    shared: Arc<(Mutex<DeadlineState>, Condvar)>,
}

struct DeadlineState {
    finished: bool,
    expired: bool,
    waker: Option<Waker>,
}

impl Deadline {
    fn start(bound: Duration) -> Self {
        let shared = Arc::new((
            Mutex::new(DeadlineState {
                finished: false,
                expired: false,
                waker: None,
            }),
            Condvar::new(),
        ));
        let deadline = Self {
            shared: Arc::clone(&shared),
        };
        if thread::Builder::new()
            .name("nsh-wake-lock-deadline".to_owned())
            .spawn(move || expire_after(shared, bound))
            .is_err()
        {
            // With no timer thread the call cannot be bounded, so it must fail, never hang.
            let mut state = deadline.state();
            state.expired = true;
        }
        deadline
    }

    fn state(&self) -> MutexGuard<'_, DeadlineState> {
        self.shared.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the deadline passed; also takes the waker to call once it does.
    fn expired(&self, cx: &mut Context<'_>) -> bool {
        let mut state = self.state();
        if state.expired {
            return true;
        }
        state.waker = Some(cx.waker().clone());
        false
    }

    /// Ends the deadline: the call answered, so the timer thread can stop.
    fn finish(&self) {
        let mut state = self.state();
        state.finished = true;
        state.waker = None;
        drop(state);
        self.shared.1.notify_all();
    }
}

/// The timer thread body: sleeps until the bound passes or the call finishes, then wakes the
/// waiting task.
fn expire_after(shared: Arc<(Mutex<DeadlineState>, Condvar)>, bound: Duration) {
    let (mutex, condvar) = &*shared;
    let deadline = Instant::now() + bound;
    let mut state = mutex.lock().unwrap_or_else(PoisonError::into_inner);
    while !state.finished && Instant::now() < deadline {
        let (guard, _) = condvar
            .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(PoisonError::into_inner);
        state = guard;
    }
    if !state.finished {
        state.expired = true;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

/// Takes one inhibit per requested flag, mirroring keepawake's Linux bridge: the display goes
/// through the session bus's screensaver, idle and sleep through logind on the system bus.
async fn acquire_inhibits(options: &WakeLockOptions) -> Result<HeldLock, WakeLockError> {
    let mut held = HeldLock {
        display: None,
        idle_fd: None,
        sleep_fd: None,
    };
    let outcome = async {
        if options.display {
            held.display = Some(inhibit_display(options).await?);
        }
        if options.idle || options.sleep {
            let who = options.app_name.as_deref().unwrap_or(DEFAULT_APP_NAME);
            let why = options.reason.as_deref().unwrap_or(DEFAULT_REASON);
            let conn = bounded(Connection::system()).await?;
            let manager = bounded(ManagerProxy::new(&conn)).await?;
            if options.idle {
                held.idle_fd = Some(bounded(manager.inhibit("idle", who, why, "block")).await?);
            }
            if options.sleep {
                held.sleep_fd = Some(bounded(manager.inhibit("sleep", who, why, "block")).await?);
            }
        }
        Ok::<(), WakeLockError>(())
    }
    .await;
    match outcome {
        Ok(()) => Ok(held),
        Err(error) => {
            // A failed acquire leaves nothing held: what was already taken is given back.
            let _ = held.release().await;
            Err(error)
        }
    }
}

/// Inhibits the display blanking and keeps the proxy whose `UnInhibit` will end it.
async fn inhibit_display(
    options: &WakeLockOptions,
) -> Result<(ScreenSaverProxy<'static>, u32), WakeLockError> {
    let conn = bounded(Connection::session()).await?;
    let screensaver = bounded(ScreenSaverProxy::new(&conn)).await?;
    let cookie = bounded(
        screensaver.inhibit(
            options
                .app_reverse_domain
                .as_deref()
                .unwrap_or(DEFAULT_APP_REVERSE_DOMAIN),
            options.reason.as_deref().unwrap_or(DEFAULT_REASON),
        ),
    )
    .await?;
    Ok((screensaver, cookie))
}

/// Runs `UnInhibit` on its own thread and waits for it. zbus 6 ships no blocking facade, and
/// `zbus::block_on` panics nested inside another `block_on`, which a `Drop` in async code is.
fn un_inhibit_elsewhere(
    screensaver: ScreenSaverProxy<'static>,
    cookie: u32,
) -> Result<(), WakeLockError> {
    let (sender, receiver) = mpsc::channel();
    let spawned = thread::Builder::new()
        .name("nsh-wake-lock-release".to_owned())
        .spawn(move || {
            // Bounded like every other call: the thread ends within CALL_BOUND, dropping
            // the proxy and its connection, so it cannot hold the inhibit past that.
            let _ = sender.send(zbus::block_on(bounded(screensaver.un_inhibit(cookie))));
        });
    let joined = match spawned {
        Ok(_) => receiver.recv_timeout(JOIN_BOUND),
        // The release thread could not start, so the inhibit stays on the bus.
        Err(_) => return Err(WakeLockError::new(ErrorKind::Backend)),
    };
    match joined {
        Ok(answer) => answer,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Past the join bound the thread may still deliver its UnInhibit; the id is spent.
            Err(WakeLockError::new(ErrorKind::Timeout))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(WakeLockError::new(ErrorKind::Backend)),
    }
}

impl LinuxBackend {
    fn locks(&self) -> MutexGuard<'_, HashMap<LockId, HeldLock>> {
        self.locks.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Backend for LinuxBackend {
    fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        Box::pin(async {
            // The system bus carries logind, the service behind idle and sleep, so reaching
            // it means locks can be held. A missing bus is the only definitive no; every
            // other answer, a missed deadline included, reports its own code.
            match raced(Connection::system()).await {
                Ok(Ok(_)) => Ok(true),
                Ok(Err(error)) if no_bus_at_all(&error) => Ok(false),
                Ok(Err(error)) => Err(backend_error(error)),
                Err(error) => Err(error),
            }
        })
    }

    fn acquire(
        &self,
        options: &WakeLockOptions,
    ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
        // The future may borrow only this backend, so the options move in as a clone.
        let options = options.clone();
        Box::pin(async move {
            // The api layer rejects options that request nothing; such options hold nothing.
            let held = acquire_inhibits(&options).await?;
            let id = LockId::new(self.next_id.fetch_add(1, Ordering::Relaxed));
            self.locks().insert(id, held);
            Ok(id)
        })
    }

    fn release(&self, id: LockId) -> Result<(), WakeLockError> {
        let Some(held) = self.locks().remove(&id) else {
            return Err(WakeLockError::new(ErrorKind::NotAcquired));
        };
        // A logind inhibitor lock lasts exactly as long as its fd: dropping it releases.
        let HeldLock {
            display,
            idle_fd: _,
            sleep_fd: _,
        } = held;
        match display {
            None => Ok(()),
            Some((screensaver, cookie)) => un_inhibit_elsewhere(screensaver, cookie),
        }
    }

    fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        let active = !self.locks().is_empty();
        Box::pin(ready(Ok::<bool, WakeLockError>(active)))
    }
}
