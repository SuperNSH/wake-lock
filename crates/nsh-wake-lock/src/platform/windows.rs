//! The Windows bridge: `PowerCreateRequest` holds one request per lock, `PowerSetRequest` turns
//! on the types the options set, and release clears each type and closes the handle.

use std::{
    collections::HashMap,
    ffi::c_void,
    future::ready,
    iter::once,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use windows::{
    Win32::Foundation::{CloseHandle, HANDLE},
    Win32::System::Power::{
        POWER_REQUEST_TYPE, PowerClearRequest, PowerCreateRequest, PowerRequestAwayModeRequired,
        PowerRequestDisplayRequired, PowerRequestSystemRequired, PowerSetRequest,
    },
    Win32::System::Threading::{
        POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
    },
    core::{Error as WindowsError, PWSTR},
};

use crate::{
    backend::{Backend, BackendFuture, LockId},
    error::{ErrorKind, WakeLockError},
    options::WakeLockOptions,
};

/// The power-request backend, tracking one handle per lock it issued.
pub(crate) fn backend() -> Arc<dyn Backend> {
    Arc::new(PowerBackend {
        state: Mutex::new(State::default()),
    })
}

struct PowerBackend {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    next_id: u64,
    held: HashMap<LockId, HeldRequest>,
}

/// One acquired lock: the request handle and the types `PowerSetRequest` turned on. The handle is
/// stored as its pointer value so the state stays `Send` and `Sync`.
struct HeldRequest {
    handle: u64,
    requests: Vec<POWER_REQUEST_TYPE>,
}

impl HeldRequest {
    fn handle(&self) -> HANDLE {
        HANDLE(self.handle as *mut c_void)
    }
}

impl PowerBackend {
    fn lock(&self) -> MutexGuard<'_, State> {
        // Poisoning only means a call failed mid-flight; the state is still consistent.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The failed call's own code, as the `Backend` error's status.
fn backend_error(error: WindowsError) -> WakeLockError {
    WakeLockError::new(ErrorKind::Backend).with_status(error.code().0 as i64)
}

fn create_request(options: &WakeLockOptions) -> Result<HeldRequest, WakeLockError> {
    // Windows takes a single reason string, so domain, app name and reason join into one, each
    // with the spec's default.
    let reason = format!(
        "{}:{}:{}",
        options
            .app_reverse_domain
            .as_deref()
            .unwrap_or("io.github.supernsh.wake-lock"),
        options.app_name.as_deref().unwrap_or("nsh-wake-lock"),
        options.reason.as_deref().unwrap_or("User requested"),
    );
    let mut wide = reason.encode_utf16().chain(once(0)).collect::<Vec<u16>>();
    let context = REASON_CONTEXT {
        // POWER_REQUEST_CONTEXT_VERSION is 0 in the SDK headers.
        Version: 0,
        Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
        Reason: REASON_CONTEXT_0 {
            SimpleReasonString: PWSTR(wide.as_mut_ptr()),
        },
    };

    // Every call here is a synchronous kernel call that returns immediately; nothing waits.
    let handle = unsafe { PowerCreateRequest(&context) }.map_err(backend_error)?;

    // The reference maps display, idle and sleep to its display, system and away-mode flags; the
    // request types follow one-for-one.
    let mut requests = Vec::new();
    if options.display {
        requests.push(PowerRequestDisplayRequired);
    }
    if options.idle {
        requests.push(PowerRequestSystemRequired);
    }
    if options.sleep {
        requests.push(PowerRequestAwayModeRequired);
    }

    for (index, &request_type) in requests.iter().enumerate() {
        if let Err(error) = unsafe { PowerSetRequest(handle, request_type) } {
            let failure = backend_error(error);
            // Rollback is best effort: clear what was set, close; the failure is the answer.
            unsafe {
                for &set_type in &requests[..index] {
                    let _ = PowerClearRequest(handle, set_type);
                }
                let _ = CloseHandle(handle);
            }
            return Err(failure);
        }
    }

    Ok(HeldRequest {
        handle: handle.0 as u64,
        requests,
    })
}

/// Clears every request type and closes the handle; the first platform failure is the answer.
fn clear_request(request: &HeldRequest) -> Result<(), WakeLockError> {
    let handle = request.handle();
    let mut failure = None;
    for &request_type in &request.requests {
        if let Err(error) = unsafe { PowerClearRequest(handle, request_type) } {
            failure.get_or_insert(backend_error(error));
        }
    }
    // Closing the handle also ends any request a failed clear left behind.
    let closed = unsafe { CloseHandle(handle) };
    if let Some(error) = failure {
        return Err(error);
    }
    closed.map_err(backend_error)
}

impl Backend for PowerBackend {
    fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        // Every Windows this builds for, 7 and later, carries the power request API.
        Box::pin(ready(Ok::<bool, WakeLockError>(true)))
    }

    fn acquire(
        &self,
        options: &WakeLockOptions,
    ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
        let answer = match create_request(options) {
            Ok(request) => {
                let mut state = self.lock();
                let id = LockId::new(state.next_id);
                state.next_id += 1;
                state.held.insert(id, request);
                Ok(id)
            }
            Err(error) => Err(error),
        };
        Box::pin(ready(answer))
    }

    fn release(&self, id: LockId) -> Result<(), WakeLockError> {
        // Off the map before the platform calls: a second release answers NotAcquired, so the
        // id resolves exactly once.
        let Some(request) = self.lock().held.remove(&id) else {
            return Err(WakeLockError::new(ErrorKind::NotAcquired));
        };
        clear_request(&request)
    }

    fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        // Windows has no per-handle query; a tracked, unreleased request is held.
        let active = !self.lock().held.is_empty();
        Box::pin(ready(Ok::<bool, WakeLockError>(active)))
    }
}
