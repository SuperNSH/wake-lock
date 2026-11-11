//! The Apple bridge: `IOPMAssertion` assertions on macOS, the idle timer on iOS. macOS mirrors
//! keepawake-rs; iOS mirrors wakelock_plus.

#[cfg(target_os = "ios")]
pub(crate) use ios::backend;
#[cfg(target_os = "macos")]
pub(crate) use macos::backend;

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        collections::HashMap,
        future::{poll_fn, ready},
        sync::{Arc, Mutex, MutexGuard, PoisonError},
        task::Poll,
    };

    use core_foundation::{
        base::TCFType,
        string::{CFString, CFStringRef},
    };

    use crate::{
        backend::{Backend, BackendFuture, LockId},
        error::{ErrorKind, WakeLockError},
        options::WakeLockOptions,
    };

    /// The macOS backend; the re-export above makes it `apple::backend`.
    pub(crate) fn backend() -> Arc<dyn Backend> {
        Arc::new(MacosBackend {
            locks: Mutex::new(Locks::default()),
        })
    }

    struct MacosBackend {
        locks: Mutex<Locks>,
    }

    #[derive(Default)]
    struct Locks {
        next_id: u64,
        held: HashMap<LockId, Assertions>,
    }

    /// One lock's assertions; `NO_ASSERTION` marks one this lock never created.
    #[derive(Clone, Copy, Default)]
    struct Assertions {
        display: u32,
        idle: u32,
        sleep: u32,
    }

    impl Backend for MacosBackend {
        fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            Box::pin(ready(Ok::<bool, WakeLockError>(true)))
        }

        fn acquire(
            &self,
            options: &WakeLockOptions,
        ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
            // Poll-lazy: the assertions are taken on the first poll, never at the call, so a
            // future dropped unpolled holds nothing that would need releasing. The clone keeps
            // the future borrowed from `self` alone, as the trait's seam requires.
            let options = options.clone();
            Box::pin(poll_fn(move |_| Poll::Ready(self.acquire_lock(&options))))
        }

        fn release(&self, id: LockId) -> Result<(), WakeLockError> {
            self.release_lock(id)
        }

        fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            let active = !self.locks().held.is_empty();
            Box::pin(ready(Ok::<bool, WakeLockError>(active)))
        }
    }

    impl MacosBackend {
        fn locks(&self) -> MutexGuard<'_, Locks> {
            self.locks.lock().unwrap_or_else(PoisonError::into_inner)
        }

        fn acquire_lock(&self, options: &WakeLockOptions) -> Result<LockId, WakeLockError> {
            let name = CFString::new(options.reason.as_deref().unwrap_or(DEFAULT_REASON));
            let mut assertions = Assertions::default();
            if let Err(error) = create_requested(options, &name, &mut assertions) {
                // A failed acquire undoes its half-created assertions, best-effort.
                let _ = release_assertions(assertions);
                return Err(error);
            }
            let mut locks = self.locks();
            let id = LockId::new(locks.next_id);
            locks.next_id += 1;
            locks.held.insert(id, assertions);
            Ok(id)
        }

        fn release_lock(&self, id: LockId) -> Result<(), WakeLockError> {
            let assertions = {
                let mut locks = self.locks();
                locks.held.remove(&id)
            };
            match assertions {
                Some(assertions) => release_assertions(assertions),
                None => Err(WakeLockError::new(ErrorKind::NotAcquired)),
            }
        }
    }

    const DEFAULT_REASON: &str = "User requested";

    type IOReturn = std::os::raw::c_int;
    type IOPMAssertionLevel = u32;
    type IOPMAssertionID = u32;

    const IO_RETURN_SUCCESS: IOReturn = 0;
    const NO_ASSERTION: IOPMAssertionID = 0;
    const ASSERTION_LEVEL_ON: IOPMAssertionLevel = 255;
    const ASSERTION_TYPE_PREVENT_USER_IDLE_DISPLAY_SLEEP: &str = "PreventUserIdleDisplaySleep";
    const ASSERTION_TYPE_PREVENT_USER_IDLE_SYSTEM_SLEEP: &str = "PreventUserIdleSystemSleep";
    const ASSERTION_TYPE_PREVENT_SYSTEM_SLEEP: &str = "PreventSystemSleep";

    fn create_requested(
        options: &WakeLockOptions,
        name: &CFString,
        into: &mut Assertions,
    ) -> Result<(), WakeLockError> {
        if options.display {
            into.display = create_assertion(ASSERTION_TYPE_PREVENT_USER_IDLE_DISPLAY_SLEEP, name)?;
        }
        if options.idle {
            into.idle = create_assertion(ASSERTION_TYPE_PREVENT_USER_IDLE_SYSTEM_SLEEP, name)?;
        }
        if options.sleep {
            into.sleep = create_assertion(ASSERTION_TYPE_PREVENT_SYSTEM_SLEEP, name)?;
        }
        Ok(())
    }

    /// Creates one assertion; a nonzero `IOReturn` becomes `Backend` carrying the code.
    fn create_assertion(
        assertion_type: &'static str,
        name: &CFString,
    ) -> Result<IOPMAssertionID, WakeLockError> {
        let assertion_type = CFString::from_static_string(assertion_type);
        let mut id: IOPMAssertionID = NO_ASSERTION;
        // SAFETY: Apple's documented signature; both strings outlive the call and `id` is a valid
        // out-pointer.
        let result = unsafe {
            IOPMAssertionCreateWithName(
                assertion_type.as_concrete_TypeRef(),
                ASSERTION_LEVEL_ON,
                name.as_concrete_TypeRef(),
                &mut id,
            )
        };
        if result != IO_RETURN_SUCCESS {
            return Err(WakeLockError::new(ErrorKind::Backend).with_status(result as i64));
        }
        Ok(id)
    }

    /// Releases every assertion of a lock, then reports the first nonzero `IOReturn`, if any.
    fn release_assertions(assertions: Assertions) -> Result<(), WakeLockError> {
        let mut first_failure = None;
        for id in [assertions.display, assertions.idle, assertions.sleep] {
            if id == NO_ASSERTION {
                continue;
            }
            // SAFETY: Apple's documented signature; the id came from IOPMAssertionCreateWithName.
            let result = unsafe { IOPMAssertionRelease(id) };
            if result != IO_RETURN_SUCCESS && first_failure.is_none() {
                first_failure =
                    Some(WakeLockError::new(ErrorKind::Backend).with_status(result as i64));
            }
        }
        match first_failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    // The IOKit symbols we need, declared by hand as keepawake-rs does; debug with
    // `pmset -g assertions`.
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: CFStringRef,
            assertion_level: IOPMAssertionLevel,
            assertion_name: CFStringRef,
            assertion_id: *mut IOPMAssertionID,
        ) -> IOReturn;
        fn IOPMAssertionRelease(assertion_id: IOPMAssertionID) -> IOReturn;
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use std::{
        future::Future,
        os::raw::{c_char, c_long, c_void},
        pin::Pin,
        ptr::null_mut,
        sync::{Arc, Mutex, MutexGuard, PoisonError},
        task::{Context, Poll, Waker},
    };

    use crate::{
        backend::{Backend, BackendFuture, LockId},
        error::{ErrorKind, WakeLockError},
        options::WakeLockOptions,
    };

    /// The iOS backend; the re-export above makes it `apple::backend`.
    pub(crate) fn backend() -> Arc<dyn Backend> {
        Arc::new(IosBackend {
            state: Arc::new(Mutex::new(State::default())),
        })
    }

    struct IosBackend {
        state: Arc<Mutex<State>>,
    }

    #[derive(Default)]
    struct State {
        next_id: u64,
        // The bool says whether the lock drives the idle timer; only display locks do.
        held: Vec<(LockId, bool)>,
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    impl Backend for IosBackend {
        fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            self.read_on_main(MainRead::AppPresent)
        }

        fn acquire(
            &self,
            options: &WakeLockOptions,
        ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
            let answer = Arc::new(Answer::empty());
            let undo_state = Arc::clone(&self.state);
            // Runs from Drop with a result that arrived after the caller gave up; the job
            // undoes itself when it sees the cancel first, so this fires only then.
            let undo: Box<dyn FnOnce(Result<LockId, WakeLockError>) + Send> =
                Box::new(move |result| {
                    if let Ok(id) = result {
                        let work = MainWork {
                            state: undo_state,
                            job: MainJob::Undo { id },
                        };
                        dispatch_main(Box::into_raw(Box::new(work)).cast(), run_main_work);
                    }
                });
            let work = MainWork {
                state: Arc::clone(&self.state),
                job: MainJob::Acquire {
                    display: options.display,
                    answer: Arc::clone(&answer),
                },
            };
            dispatch_main(Box::into_raw(Box::new(work)).cast(), run_main_work);
            Box::pin(AnswerFuture {
                answer,
                undo: Some(undo),
                resolved: false,
            })
        }

        fn release(&self, id: LockId) -> Result<(), WakeLockError> {
            // The removal and the timer's fate decide in one section; the dispatch itself stays
            // outside the mutex.
            let reconcile = {
                let mut state = lock(&self.state);
                let Some(index) = state.held.iter().position(|&(held, _)| held == id) else {
                    return Err(WakeLockError::new(ErrorKind::NotAcquired));
                };
                let (_, drove) = state.held.swap_remove(index);
                drove && !state.held.iter().any(|&(_, drives)| drives)
            };
            if reconcile {
                let work = MainWork {
                    state: Arc::clone(&self.state),
                    job: MainJob::Reconcile,
                };
                dispatch_main(Box::into_raw(Box::new(work)).cast(), run_main_work);
            }
            Ok(())
        }

        fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            self.read_on_main(MainRead::TimerDisabled)
        }
    }

    impl IosBackend {
        fn read_on_main(&self, read: MainRead) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            let answer = Arc::new(Answer::empty());
            let work = MainWork {
                state: Arc::clone(&self.state),
                job: MainJob::Read {
                    read,
                    answer: Arc::clone(&answer),
                },
            };
            dispatch_main(Box::into_raw(Box::new(work)).cast(), run_main_work);
            Box::pin(AnswerFuture {
                answer,
                undo: None,
                resolved: false,
            })
        }
    }

    /// One unit of main-queue work; UIKit permits its selectors on the main thread only.
    struct MainWork {
        state: Arc<Mutex<State>>,
        job: MainJob,
    }

    enum MainJob {
        /// Takes a lock: the display flag drives the idle timer, and the answer carries the
        /// acquire's result.
        Acquire {
            display: bool,
            answer: Arc<Answer<Result<LockId, WakeLockError>>>,
        },
        /// A bool read: whether an app context exists, or the live timer state.
        Read {
            read: MainRead,
            answer: Arc<Answer<Result<bool, WakeLockError>>>,
        },
        /// Puts the idle timer back in line with the held locks.
        Reconcile,
        /// Gives back a lock whose acquire the caller cancelled after it was taken.
        Undo { id: LockId },
    }

    enum MainRead {
        AppPresent,
        TimerDisabled,
    }

    extern "C" fn run_main_work(context: *mut c_void) {
        // SAFETY: dispatch_main boxed this MainWork and the queue runs it exactly once, so
        // from_raw re-owns the box and drops it here.
        let work = unsafe { *Box::from_raw(context.cast::<MainWork>()) };
        run_job(&work.state, work.job);
    }

    fn run_job(state: &Mutex<State>, job: MainJob) {
        let app = shared_application();
        match job {
            MainJob::Acquire { display, answer } => {
                let result = if app.is_null() {
                    Err(WakeLockError::new(ErrorKind::NotSupported))
                } else if application_state(app) == APPLICATION_STATE_BACKGROUND {
                    Err(WakeLockError::new(ErrorKind::Backgrounded))
                } else {
                    Ok(acquire_on_main(state, app, display))
                };
                // Publishing is the decision point: a cancel seen here hands the result back so
                // this job undoes the lock it just took; nothing is orphaned either way.
                if let Delivery::Cancelled(Ok(id)) = answer.publish(result) {
                    undo_on_main(state, app, id);
                }
            }
            MainJob::Read { read, answer } => {
                let value = match read {
                    MainRead::AppPresent => !app.is_null(),
                    MainRead::TimerDisabled => !app.is_null() && idle_timer_disabled(app),
                };
                // A cancelled read leaves nothing behind, so the value simply goes nowhere.
                let _ = answer.publish(Ok(value));
            }
            MainJob::Reconcile => {
                if !app.is_null() {
                    reconcile_timer(state, app);
                }
            }
            MainJob::Undo { id } => undo_on_main(state, app, id),
        }
    }

    fn acquire_on_main(state: &Mutex<State>, app: Id, display: bool) -> LockId {
        // The push and the timer's decision share one critical section, and the serial queue
        // wraps the whole item in another, so a release cannot interleave the two.
        let (id, drives) = {
            let mut state = lock(state);
            let id = LockId::new(state.next_id);
            state.next_id += 1;
            state.held.push((id, display));
            (id, state.held.iter().any(|&(_, drives)| drives))
        };
        if drives && !idle_timer_disabled(app) {
            set_idle_timer_disabled(app, true);
        }
        id
    }

    /// Takes back a lock the caller never received: forget the id, then line the timer up with
    /// whatever remains.
    fn undo_on_main(state: &Mutex<State>, app: Id, id: LockId) {
        lock(state).held.retain(|&(held, _)| held != id);
        if !app.is_null() {
            reconcile_timer(state, app);
        }
    }

    /// Matches the timer to the held locks, reading them at run time so every reconcile
    /// converges whatever order the queue runs the work in.
    fn reconcile_timer(state: &Mutex<State>, app: Id) {
        let drives = lock(state).held.iter().any(|&(_, drives)| drives);
        if drives != idle_timer_disabled(app) {
            set_idle_timer_disabled(app, drives);
        }
    }

    /// What a dispatched job reports back with: the main queue fills it, the future polls it.
    struct Answer<T> {
        state: Mutex<AnswerState<T>>,
    }

    struct AnswerState<T> {
        value: Option<T>,
        waker: Option<Waker>,
        cancelled: bool,
    }

    /// Which side of the cancel line a published value landed on.
    enum Delivery<T> {
        Published,
        Cancelled(T),
    }

    impl<T> Answer<T> {
        fn empty() -> Self {
            Self {
                state: Mutex::new(AnswerState {
                    value: None,
                    waker: None,
                    cancelled: false,
                }),
            }
        }

        /// Publishes and wakes the future, or hands the value back untouched when the caller
        /// already cancelled — one mutex section decides, so the two paths never overlap.
        fn publish(&self, value: T) -> Delivery<T> {
            let waker = {
                let mut state = lock(&self.state);
                if state.cancelled {
                    return Delivery::Cancelled(value);
                }
                state.value = Some(value);
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
            Delivery::Published
        }

        /// The caller gave up: mark the cancel, and hand back a value that already arrived.
        fn abandon(&self) -> Option<T> {
            let mut state = lock(&self.state);
            state.cancelled = true;
            state.value.take()
        }
    }

    /// The caller's half of an Answer; resolves exactly once, and a drop before that is a cancel
    /// that undoes whatever the job did.
    struct AnswerFuture<T> {
        answer: Arc<Answer<T>>,
        // Runs from Drop with a value that arrived after the caller gave up; None when the job's
        // own cancel check undoes instead.
        undo: Option<Box<dyn FnOnce(T) + Send>>,
        resolved: bool,
    }

    impl<T> Future for AnswerFuture<T> {
        type Output = T;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
            let this = self.get_mut();
            let mut state = lock(&this.answer.state);
            if let Some(value) = state.value.take() {
                this.resolved = true;
                return Poll::Ready(value);
            }
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    impl<T> Drop for AnswerFuture<T> {
        fn drop(&mut self) {
            if self.resolved {
                return;
            }
            if let Some(value) = self.answer.abandon()
                && let Some(undo) = self.undo.take()
            {
                undo(value);
            }
        }
    }

    /// An Objective-C object; classes are objects too.
    #[repr(C)]
    struct Object {
        _opaque: [u8; 0],
    }

    type Id = *mut Object;
    type Sel = *const c_void;

    const APPLICATION_STATE_BACKGROUND: c_long = 2;

    // dispatch_async_f takes a plain function pointer and context, so no Block machinery is
    // needed; libdispatch is part of libSystem, linked on every Apple target.
    unsafe extern "C" {
        fn dispatch_get_main_queue() -> *mut DispatchQueue;
        fn dispatch_async_f(
            queue: *mut DispatchQueue,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
    }

    /// A libdispatch queue; opaque to us.
    #[repr(C)]
    struct DispatchQueue {
        _opaque: [u8; 0],
    }

    /// Runs work on the main queue, the only thread UIKit permits, without blocking the caller.
    /// The queue stays undrained only if the main run loop is dead — the same assumption
    /// wakelock_plus makes of the main thread.
    fn dispatch_main(context: *mut c_void, work: extern "C" fn(*mut c_void)) {
        // SAFETY: `context` is a Box the work fn re-owns; dispatch_async_f queues and returns
        // immediately, and never rejects work.
        unsafe {
            dispatch_async_f(dispatch_get_main_queue(), context, work);
        }
    }

    // The few libobjc symbols the idle-timer calls need, declared by hand: the crate carries no
    // objc2 dependency.
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
    }

    // "C-unwind": an Objective-C exception unwinds out definedly — aborting the process if
    // nothing catches it — instead of UB through these nounwind frames.
    #[link(name = "objc")]
    unsafe extern "C-unwind" {
        fn objc_msgSend();
    }

    fn shared_application() -> Id {
        // SAFETY: the literals are NUL-terminated class and selector names; a null class means
        // UIKit is not loaded, reported by the callers as NotSupported.
        unsafe {
            let class = objc_getClass(c"UIApplication".as_ptr());
            if class.is_null() {
                return null_mut();
            }
            msg_send_id(class, sel_registerName(c"sharedApplication".as_ptr()))
        }
    }

    fn application_state(app: Id) -> c_long {
        // SAFETY: `applicationState` takes no arguments and returns UIApplicationState, an
        // NSInteger.
        unsafe { msg_send_c_long(app, sel_registerName(c"applicationState".as_ptr())) }
    }

    fn idle_timer_disabled(app: Id) -> bool {
        // SAFETY: `isIdleTimerDisabled` takes no arguments and returns BOOL, a signed char on
        // Apple platforms.
        unsafe { msg_send_c_char(app, sel_registerName(c"isIdleTimerDisabled".as_ptr())) != 0 }
    }

    fn set_idle_timer_disabled(app: Id, disabled: bool) {
        // SAFETY: `setIdleTimerDisabled:` takes one BOOL and returns void; only the main queue
        // reaches here.
        unsafe {
            msg_send_set_c_char(
                app,
                sel_registerName(c"setIdleTimerDisabled:".as_ptr()),
                disabled as c_char,
            )
        }
    }

    // objc_msgSend has no single Rust signature; every call needs its own typed view of the same
    // primitive, which keeps the receiver and selector in the two leading argument registers.
    fn msg_send_id(receiver: Id, selector: Sel) -> Id {
        // SAFETY: only `sharedApplication` rides this view, which returns an object reference.
        unsafe {
            let send: unsafe extern "C-unwind" fn(Id, Sel) -> Id = std::mem::transmute::<
                unsafe extern "C-unwind" fn(),
                unsafe extern "C-unwind" fn(Id, Sel) -> Id,
            >(objc_msgSend);
            send(receiver, selector)
        }
    }

    fn msg_send_c_long(receiver: Id, selector: Sel) -> c_long {
        // SAFETY: only `applicationState` rides this view, which returns an NSInteger.
        unsafe {
            let send: unsafe extern "C-unwind" fn(Id, Sel) -> c_long = std::mem::transmute::<
                unsafe extern "C-unwind" fn(),
                unsafe extern "C-unwind" fn(Id, Sel) -> c_long,
            >(objc_msgSend);
            send(receiver, selector)
        }
    }

    fn msg_send_c_char(receiver: Id, selector: Sel) -> c_char {
        // SAFETY: only `isIdleTimerDisabled` rides this view, which returns a BOOL.
        unsafe {
            let send: unsafe extern "C-unwind" fn(Id, Sel) -> c_char = std::mem::transmute::<
                unsafe extern "C-unwind" fn(),
                unsafe extern "C-unwind" fn(Id, Sel) -> c_char,
            >(objc_msgSend);
            send(receiver, selector)
        }
    }

    fn msg_send_set_c_char(receiver: Id, selector: Sel, argument: c_char) {
        // SAFETY: only `setIdleTimerDisabled:` rides this view: one BOOL argument, void return.
        unsafe {
            let send: unsafe extern "C-unwind" fn(Id, Sel, c_char) = std::mem::transmute::<
                unsafe extern "C-unwind" fn(),
                unsafe extern "C-unwind" fn(Id, Sel, c_char),
            >(objc_msgSend);
            send(receiver, selector, argument)
        }
    }
}
