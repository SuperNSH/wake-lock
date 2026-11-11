//! The Android bridge: `FLAG_KEEP_SCREEN_ON` on the current activity's window, over JNI, as
//! `wakelock_plus` toggles it.

pub(crate) use window::backend;

mod window {
    use std::{
        collections::HashMap,
        ffi::c_void,
        future::ready,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc, Mutex, MutexGuard, PoisonError,
            atomic::{AtomicBool, Ordering},
        },
    };

    use jni::{
        Env, JavaVM, jni_sig, jni_str,
        objects::{JObject, JValue},
        sys::jint,
    };
    use ndk_context::AndroidContext;

    use crate::{
        backend::{Backend, BackendFuture, LockId},
        error::{ErrorKind, WakeLockError},
        options::WakeLockOptions,
    };

    /// `WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON`.
    const FLAG_KEEP_SCREEN_ON: jint = 0x0000_0080;

    /// The Android backend; the re-export above makes it `android::backend`.
    pub(crate) fn backend() -> Arc<dyn Backend> {
        Arc::new(AndroidBackend {
            locks: Mutex::new(Locks::default()),
        })
    }

    struct AndroidBackend {
        locks: Mutex<Locks>,
    }

    /// One entry per held lock: whether it asked to keep the display on. `idle` and `sleep` are
    /// accepted (Android has no channel for them) and so hold bookkeeping only.
    #[derive(Default)]
    struct Locks {
        next_id: u64,
        held: HashMap<LockId, bool>,
        /// A failed clear left the window flag physically on; is_active must not hide it, and the
        /// next last-display release retries the clear.
        flag_stuck: bool,
    }

    impl From<jni::errors::Error> for WakeLockError {
        fn from(error: jni::errors::Error) -> Self {
            match error {
                // A raw call status is the only code a jni error carries; the rest map bare.
                jni::errors::Error::JniCall(jni::errors::JniError::Other(code)) => {
                    WakeLockError::new(ErrorKind::Backend).with_status(code as i64)
                }
                _ => WakeLockError::new(ErrorKind::Backend),
            }
        }
    }

    impl Backend for AndroidBackend {
        fn is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            let supported = activity_present();
            Box::pin(ready(Ok::<bool, WakeLockError>(supported)))
        }

        fn acquire(
            &self,
            options: &WakeLockOptions,
        ) -> BackendFuture<'_, Result<LockId, WakeLockError>> {
            let answer = self.acquire_lock(options);
            Box::pin(ready(answer))
        }

        fn release(&self, id: LockId) -> Result<(), WakeLockError> {
            self.release_lock(id)
        }

        fn is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>> {
            let locks = self.locks();
            let active = !locks.held.is_empty() || locks.flag_stuck;
            Box::pin(ready(Ok::<bool, WakeLockError>(active)))
        }
    }

    impl AndroidBackend {
        fn locks(&self) -> MutexGuard<'_, Locks> {
            self.locks.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// The window flag is one resource every display lock shares, so its change sits inside
        /// the same lock as the bookkeeping: on with the first display lock, off with the last.
        fn acquire_lock(&self, options: &WakeLockOptions) -> Result<LockId, WakeLockError> {
            let mut locks = self.locks();
            if options.display && !locks.any_display() && !locks.flag_stuck {
                keep_screen_on(true)?;
            }
            let id = LockId::new(locks.next_id);
            locks.next_id += 1;
            locks.held.insert(id, options.display);
            Ok(id)
        }

        fn release_lock(&self, id: LockId) -> Result<(), WakeLockError> {
            let mut locks = self.locks();
            let Some(display) = locks.held.remove(&id) else {
                return Err(WakeLockError::new(ErrorKind::NotAcquired));
            };
            if display && !locks.any_display() {
                // The last display lock went: clear, or record that the flag stayed on, so the
                // truth stays observable and the next drain retries.
                match keep_screen_on(false) {
                    Ok(()) => locks.flag_stuck = false,
                    Err(error) => {
                        locks.flag_stuck = true;
                        return Err(error);
                    }
                }
            }
            Ok(())
        }
    }

    impl Locks {
        /// Whether any held lock keeps the display on; the window flag follows this edge.
        fn any_display(&self) -> bool {
            self.held.values().any(|display| *display)
        }
    }

    /// The process's Android context, or none while no runtime has installed one. `ndk-context`'s
    /// getter panics until then and has no try-get, so the probe is caught; a context, once seen,
    /// is never taken back.
    fn context() -> Option<AndroidContext> {
        static SEEN: AtomicBool = AtomicBool::new(false);
        static PROBE: Mutex<()> = Mutex::new(());
        if SEEN.load(Ordering::Acquire) {
            return Some(ndk_context::android_context());
        }
        // One probe at a time: a take inside another probe's silent window would restore the
        // silent hook process-wide.
        let _probe = PROBE.lock().unwrap_or_else(PoisonError::into_inner);
        if SEEN.load(Ordering::Acquire) {
            return Some(ndk_context::android_context());
        }
        // A panicking probe would log; a silent hook for its duration keeps that out.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let probe = catch_unwind(AssertUnwindSafe(ndk_context::android_context));
        std::panic::set_hook(previous);
        match probe {
            Ok(context) => {
                SEEN.store(true, Ordering::Release);
                Some(context)
            }
            Err(_) => None,
        }
    }

    /// Runs `work` attached to the process's VM, handing it the installed context pointer. No
    /// context or VM is `NoActivity`.
    fn with_context<R>(
        work: impl FnOnce(&mut Env<'_>, *mut c_void) -> Result<R, WakeLockError>,
    ) -> Result<R, WakeLockError> {
        let Some(context) = context() else {
            return Err(WakeLockError::new(ErrorKind::NoActivity));
        };
        if context.vm().is_null() {
            return Err(WakeLockError::new(ErrorKind::NoActivity));
        }
        // SAFETY: ndk-context hands the runtime's own JavaVM, valid for the process's lifetime.
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
        vm.attach_current_thread(|env| work(env, context.context()))
    }

    /// The installed context as an activity. A null pointer, or no activity, is `NoActivity`.
    fn activity<'local>(
        env: &mut Env<'local>,
        raw_context: *mut c_void,
    ) -> Result<JObject<'local>, WakeLockError> {
        if raw_context.is_null() {
            return Err(WakeLockError::new(ErrorKind::NoActivity));
        }
        // SAFETY: the runtime installed this reference for the process's lifetime, and the
        // wrapper owns nothing.
        let activity = unsafe { JObject::from_raw(env, raw_context.cast()) };
        if !env.is_instance_of(&activity, jni_str!("android/app/Activity"))? {
            return Err(WakeLockError::new(ErrorKind::NoActivity));
        }
        Ok(activity)
    }

    /// Whether the installed context is an activity: the one window the flag can land on. A
    /// failed probe answers false; support is a fact, not an error.
    fn activity_present() -> bool {
        with_context(|env, raw_context| activity(env, raw_context).map(|_| ())).is_ok()
    }

    /// Sets `FLAG_KEEP_SCREEN_ON` on the current activity's window: `addFlags` on, `clearFlags`
    /// off, as `wakelock_plus`'s `applyWakelock` does.
    fn keep_screen_on(on: bool) -> Result<(), WakeLockError> {
        with_context(|env, raw_context| apply_flag(env, raw_context, on))
    }

    /// Puts the flag on the activity's window. A context that is no activity, or an activity
    /// with no window, is `NoActivity`; a JNI failure is `Backend` with the JNI status.
    fn apply_flag(
        env: &mut Env<'_>,
        raw_context: *mut c_void,
        on: bool,
    ) -> Result<(), WakeLockError> {
        let activity = activity(env, raw_context)?;
        let window = env
            .call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;
        if window.is_null() {
            return Err(WakeLockError::new(ErrorKind::NoActivity));
        }
        // The flag follows the window present at this call; a recreated activity's fresh window
        // is never re-flagged (`ndk-context` has no lifecycle hook).
        if on {
            env.call_method(
                &window,
                jni_str!("addFlags"),
                jni_sig!("(I)V"),
                &[JValue::Int(FLAG_KEEP_SCREEN_ON)],
            )?;
        } else {
            env.call_method(
                &window,
                jni_str!("clearFlags"),
                jni_sig!("(I)V"),
                &[JValue::Int(FLAG_KEEP_SCREEN_ON)],
            )?;
        }
        Ok(())
    }
}
