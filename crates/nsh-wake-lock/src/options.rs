//! The options of one wake lock.

/// The options of one lock, as `keepawake` names them. Start from [`WakeLockOptions::default`] and
/// set what you need.
///
/// Options a platform cannot apply are accepted and ignored, as `docs/parity.md` lists them. Where
/// a platform wants a name, the bridge fills the default in.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WakeLockOptions {
    /// Prevents the display from sleeping.
    pub display: bool,
    /// Prevents idle system sleep while the app runs.
    pub idle: bool,
    /// Prevents system sleep outright.
    pub sleep: bool,
    /// Why the app holds the lock, shown by tools such as `pmset -g assertions`.
    /// Defaults to "User requested".
    pub reason: Option<String>,
    /// The app's display name. Defaults to "nsh-wake-lock".
    pub app_name: Option<String>,
    /// The app's reverse domain, Android's convention. Defaults to "io.github.supernsh.wake-lock".
    pub app_reverse_domain: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_requests_no_lock_and_names_nothing() {
        let options = WakeLockOptions::default();

        assert!(!options.display);
        assert!(!options.idle);
        assert!(!options.sleep);
        assert_eq!(options.reason, None);
        assert_eq!(options.app_name, None);
        assert_eq!(options.app_reverse_domain, None);
    }
}
