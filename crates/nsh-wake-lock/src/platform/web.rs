//! The web bridge: the Screen Wake Lock API through `wasm-bindgen` and `js-sys`. The web has
//! one lock, the screen; the browser revokes it when the page hides, which the sentinel's own
//! `release` event reports, as the TS bridge in `.refs` does. `platform/mod.rs` builds this
//! file on `wasm32` only; every other target lands on another bridge.

use std::{
    future::{Future, ready},
    pin::Pin,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};

use js_sys::{Array, Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use crate::{
    backend::{Backend, BackendFuture, LockId},
    error::{ErrorKind, WakeLockError},
    options::WakeLockOptions,
};

/// How long `acquire` waits on the browser's request before answering `Timeout`; the request
/// promise itself carries no deadline.
const REQUEST_TIMEOUT_MS: f64 = 10_000.0;

/// The rejection value the timeout timer fires, told apart from the browser's own rejection
/// reasons (`AbortError`, `NotAllowedError`).
const TIMEOUT_MARKER: &str = "nsh-wake-lock-request-timeout";

/// The web backend, the one `WakeLock` holds for the process.
pub(crate) fn backend() -> Arc<dyn Backend> {
    Arc::new(WebBackend {
        state: Mutex::new(WebState {
            next_id: 0,
            locks: Vec::new(),
        }),
    })
}

struct WebBackend {
    // No JS call runs under this lock: the browser re-enters Rust while JS executes.
    state: Mutex<WebState>,
}

struct WebState {
    next_id: u64,
    locks: Vec<HeldLock>,
}

struct HeldLock {
    id: LockId,
    sentinel: JsValue,
    // `Some` when the listener was attached by name and can be removed by name.
    listener: Option<JsValue>,
    // Set by the sentinel's release event; the sentinel's own `released` field backs it.
    released: Arc<AtomicBool>,
}

impl WebBackend {
    fn lock_state(&self) -> MutexGuard<'_, WebState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn acquire_screen(&self) -> Result<LockId, WakeLockError> {
        let Some(api) = wake_lock_api()? else {
            return Err(WakeLockError::new(ErrorKind::NotSupported));
        };
        let request = function_of(&api, "request")?;
        let answer = call(&request, &api, &[JsValue::from_str("screen")])
            .map_err(|_| WakeLockError::new(ErrorKind::Backend))?;
        let Some(promise) = answer.dyn_ref::<Promise>() else {
            return Err(WakeLockError::new(ErrorKind::Backend));
        };
        let sentinel = match settle(&race_with_timeout(promise), promise).await {
            Ok(sentinel) => sentinel,
            Err(reason) => return Err(error_from_rejection(&reason)),
        };
        self.keep(sentinel)
    }

    fn keep(&self, sentinel: JsValue) -> Result<LockId, WakeLockError> {
        let (released, listener) = arm_release_tracker(&sentinel);
        let mut state = self.lock_state();
        let id = LockId::new(state.next_id);
        state.next_id += 1;
        state.locks.push(HeldLock {
            id,
            sentinel,
            listener,
            released,
        });
        Ok(id)
    }
}

impl Backend for WebBackend {
    fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        // A failed probe is a backend error here, the same as acquire reports it.
        let supported = wake_lock_api().map(|api| api.is_some());
        Box::pin(ready(supported))
    }

    fn acquire(
        &self,
        _options: &WakeLockOptions,
    ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
        // The web has exactly one lock, so display, idle and sleep all request the screen.
        Box::pin(self.acquire_screen())
    }

    fn release(&self, id: LockId) -> Result<(), WakeLockError> {
        let held = {
            let mut state = self.lock_state();
            let Some(index) = state.locks.iter().position(|lock| lock.id == id) else {
                return Err(WakeLockError::new(ErrorKind::NotAcquired));
            };
            state.locks.remove(index)
        };
        detach_listener(&held.sentinel, &held.listener);
        release_sentinel(&held.sentinel)
    }

    fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
        let held: Vec<_> = {
            let state = self.lock_state();
            state
                .locks
                .iter()
                .map(|lock| (lock.sentinel.clone(), Arc::clone(&lock.released)))
                .collect()
        };
        let active = held.iter().any(|(sentinel, released)| {
            !released.load(Ordering::Relaxed) && !sentinel_reports_released(sentinel)
        });
        Box::pin(ready(Ok::<bool, WakeLockError>(active)))
    }
}

/// `navigator.wakeLock`: `None` where the host has none, a backend error where the probe
/// itself fails.
fn wake_lock_api() -> Result<Option<JsValue>, WakeLockError> {
    let navigator = property(&js_sys::global(), "navigator")?;
    if navigator.is_undefined() || navigator.is_null() {
        return Ok(None);
    }
    let api = property(&navigator, "wakeLock")?;
    if api.is_undefined() || api.is_null() {
        return Ok(None);
    }
    Ok(Some(api))
}

fn property(object: &JsValue, name: &str) -> Result<JsValue, WakeLockError> {
    Reflect::get(object, &JsValue::from_str(name))
        .map_err(|_| WakeLockError::new(ErrorKind::Backend))
}

fn function_of(object: &JsValue, name: &str) -> Result<Function, WakeLockError> {
    property(object, name)?
        .dyn_into()
        .map_err(|_| WakeLockError::new(ErrorKind::Backend))
}

/// Calls a JS function, catching any throw; every dynamic call in this bridge funnels through
/// here.
fn call(function: &Function, this: &JsValue, args: &[JsValue]) -> Result<JsValue, JsValue> {
    Reflect::apply(function, this, &args.iter().cloned().collect::<Array>())
}

/// Races the request against a timer so `acquire` cannot hang on a promise that never
/// settles.
fn race_with_timeout(request: &Promise) -> Promise {
    match timeout_promise() {
        Some(timeout) => Promise::race(&Array::of2(request, &timeout)),
        // The bound is conditional: a host without setTimeout cannot arm the timer. Real
        // browsers expose setTimeout wherever `navigator.wakeLock` exists.
        None => request.clone(),
    }
}

fn timeout_promise() -> Option<Promise> {
    let global = js_sys::global();
    let set_timeout = property(&global, "setTimeout")
        .ok()?
        .dyn_into::<Function>()
        .ok()?;
    Some(Promise::new(&mut |_resolve, reject| {
        let fire: Closure<dyn FnMut()> = Closure::new(move || {
            // Rejecting a race that already settled is a no-op, so a late timer is harmless.
            let _ = call(
                &reject,
                &JsValue::UNDEFINED,
                &[JsValue::from_str(TIMEOUT_MARKER)],
            );
        });
        // The timer keeps the callback alive; rejecting with the marker, not `undefined`, is
        // what tells a timeout apart from a browser rejection.
        let _ = call(
            &set_timeout,
            &global,
            &[fire.into_js_value(), JsValue::from_f64(REQUEST_TIMEOUT_MS)],
        );
    }))
}

/// Nothing keeps what the request grants from here on (a `Timeout`, a rejection, or a
/// cancelled acquire), so a sentinel that still shows up is released fire-and-forget, its
/// promise's rejection swallowed; a rejection of the late request is ignored.
fn discard_late_sentinel(request: &Promise) {
    let late: Closure<dyn FnMut(JsValue)> = Closure::new(move |sentinel: JsValue| {
        let _ = release_sentinel(&sentinel);
    });
    let ignore: Closure<dyn FnMut(JsValue)> = Closure::new(|_reason: JsValue| {});
    let _ = request.then2(&late, &ignore);
    let _ = late.into_js_value();
    let _ = ignore.into_js_value();
}

/// Watches the raced promise; `request` stays with the future so its drop guard can discard
/// whatever the request still grants.
fn settle(raced: &Promise, request: &Promise) -> SettleFuture {
    let settlement = Arc::new(Mutex::new(Settlement {
        outcome: None,
        waker: None,
    }));
    let fulfilled: Closure<dyn FnMut(JsValue)> = {
        let settlement = Arc::clone(&settlement);
        Closure::new(move |value: JsValue| {
            deliver(&settlement, Ok(value));
        })
    };
    let rejected: Closure<dyn FnMut(JsValue)> = {
        let settlement = Arc::clone(&settlement);
        Closure::new(move |reason: JsValue| {
            deliver(&settlement, Err(reason));
        })
    };
    let _ = raced.then2(&fulfilled, &rejected);
    let _ = fulfilled.into_js_value();
    let _ = rejected.into_js_value();
    SettleFuture {
        request: request.clone(),
        settlement,
        claimed: false,
    }
}

fn deliver(settlement: &Mutex<Settlement>, outcome: Result<JsValue, JsValue>) {
    // Waker fires after unlock: an inline re-poll from wake() must not re-lock here.
    let waker = {
        let mut settlement = settlement.lock().unwrap_or_else(PoisonError::into_inner);
        if settlement.outcome.is_some() {
            return;
        }
        settlement.outcome = Some(outcome);
        settlement.waker.take()
    };
    if let Some(waker) = waker {
        waker.wake();
    }
}

struct Settlement {
    outcome: Option<Result<JsValue, JsValue>>,
    waker: Option<Waker>,
}

struct SettleFuture {
    request: Promise,
    settlement: Arc<Mutex<Settlement>>,
    // Set once poll handed a granted sentinel to the caller, who now owns its lifetime.
    claimed: bool,
}

impl Drop for SettleFuture {
    fn drop(&mut self) {
        // Every end but a claimed sentinel (cancel, timeout, rejection) owes the request a
        // discard, attached exactly once: whatever it grants after this moment is released,
        // never orphaned.
        if !self.claimed {
            discard_late_sentinel(&self.request);
        }
    }
}

impl Future for SettleFuture {
    type Output = Result<JsValue, JsValue>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let outcome = {
            let mut settlement = self
                .settlement
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            match settlement.outcome.take() {
                Some(outcome) => outcome,
                None => {
                    settlement.waker = Some(cx.waker().clone());
                    return Poll::Pending;
                }
            }
        };
        // Only a fulfilled race claims: its sentinel is handed to the caller.
        if outcome.is_ok() {
            self.claimed = true;
        }
        Poll::Ready(outcome)
    }
}

/// Arms the sentinel's own release notification, as the TS bridge does: the event listener
/// when the sentinel has one, the `onrelease` property otherwise.
fn arm_release_tracker(sentinel: &JsValue) -> (Arc<AtomicBool>, Option<JsValue>) {
    let released = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&released);
    let listener: Closure<dyn FnMut(JsValue)> = Closure::new(move |_event: JsValue| {
        flag.store(true, Ordering::Relaxed);
    });
    let callback = listener.into_js_value();
    if let Ok(add) = property(sentinel, "addEventListener")
        && let Some(add) = add.dyn_ref::<Function>()
        && call(
            add,
            sentinel,
            &[JsValue::from_str("release"), callback.clone()],
        )
        .is_ok()
    {
        return (released, Some(callback));
    }
    // The old fallback; `None` tells `detach_listener` to clear the property.
    let _ = Reflect::set(sentinel, &JsValue::from_str("onrelease"), &callback);
    (released, None)
}

/// Stops the release notification, so a released sentinel stops holding the callback.
fn detach_listener(sentinel: &JsValue, listener: &Option<JsValue>) {
    match listener {
        Some(callback) => {
            if let Ok(remove) = property(sentinel, "removeEventListener")
                && let Some(remove) = remove.dyn_ref::<Function>()
            {
                // A failed removal is harmless: the callback is GC-owned and only flips a
                // flag nobody reads again.
                let _ = call(
                    remove,
                    sentinel,
                    &[JsValue::from_str("release"), callback.clone()],
                );
            }
        }
        None => {
            let _ = Reflect::set(sentinel, &JsValue::from_str("onrelease"), &JsValue::NULL);
        }
    }
}

/// Fires `sentinel.release()`. `release` is sync, so the promise the call returns cannot be
/// awaited here and its rejection is swallowed; only a throw at the call itself is a known
/// failure. The TS bridge fires and forgets the same way.
fn release_sentinel(sentinel: &JsValue) -> Result<(), WakeLockError> {
    let release = property(sentinel, "release")?;
    let Some(release) = release.dyn_ref::<Function>() else {
        // The TS bridge's `release?.()`: a sentinel without the method is already released.
        return Ok(());
    };
    let answer = call(release, sentinel, &[]).map_err(|reason| error_from_rejection(&reason))?;
    if let Some(promise) = answer.dyn_ref::<Promise>() {
        let swallow: Closure<dyn FnMut(JsValue)> = Closure::new(|_reason: JsValue| {});
        let _ = promise.then2(&swallow, &swallow);
        let _ = swallow.into_js_value();
    }
    Ok(())
}

/// Whether the rejection is the timeout marker this bridge fires itself.
fn is_timeout(reason: &JsValue) -> bool {
    reason
        .as_string()
        .is_some_and(|reason| reason == TIMEOUT_MARKER)
}

/// A rejection becomes `Backend`, keeping the JS error's numeric `code` when it carries one;
/// the timeout marker is the one rejection with a kind of its own.
fn error_from_rejection(reason: &JsValue) -> WakeLockError {
    if is_timeout(reason) {
        return WakeLockError::new(ErrorKind::Timeout);
    }
    let mut error = WakeLockError::new(ErrorKind::Backend);
    // A `code` of 0 means the name has no legacy code, and NaN casts saturate to 0; both
    // carry no status.
    if let Ok(code) = property(reason, "code")
        && let Some(code) = code
            .as_f64()
            .filter(|code| code.is_finite() && *code != 0.0)
    {
        error = error.with_status(code as i64);
    }
    error
}

/// The sentinel's own `released` field; unreadable counts as not released.
fn sentinel_reports_released(sentinel: &JsValue) -> bool {
    property(sentinel, "released")
        .ok()
        .and_then(|released| released.as_bool())
        .unwrap_or(false)
}
