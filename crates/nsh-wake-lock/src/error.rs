//! The errors a wake-lock call reports. Each has a stable code; a message says what to do.

use std::fmt;

/// What went wrong, as a stable code; see [`WakeLockError::code`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// This platform or browser has no wake-lock service.
    NotSupported,
    /// The options request no lock: `display`, `idle` and `sleep` are all false.
    InvalidArgument,
    /// There is no window or activity to hold the lock on right now.
    NoActivity,
    /// The app is backgrounded, so the idle-timer change does not apply now.
    Backgrounded,
    /// The platform took too long to answer.
    Timeout,
    /// That lock is not held: it was released already or never acquired.
    NotAcquired,
    /// The platform failed; `status` holds its own code where it gives one.
    Backend,
}

/// A failed wake-lock call.
///
/// `Display` names the code and the message. Neither holds the reason text or any user data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .kind.message())]
pub struct WakeLockError {
    kind: ErrorKind,
    status: Option<i64>,
}

impl WakeLockError {
    /// An error of `kind`, with no platform status.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, status: None }
    }

    /// The same error, carrying the platform's own status code.
    #[must_use]
    pub fn with_status(mut self, status: i64) -> Self {
        self.status = Some(status);
        self
    }

    /// What went wrong.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The platform's status code, when the platform gave one.
    pub fn status(&self) -> Option<i64> {
        self.status
    }

    /// The stable code, such as `ERR_WAKE_LOCK_NOT_ACQUIRED`. Apps match on this.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl ErrorKind {
    /// The stable code of this kind.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::NotSupported => "ERR_WAKE_LOCK_NOT_SUPPORTED",
            ErrorKind::InvalidArgument => "ERR_WAKE_LOCK_INVALID_ARGUMENT",
            ErrorKind::NoActivity => "ERR_WAKE_LOCK_NO_ACTIVITY",
            ErrorKind::Backgrounded => "ERR_WAKE_LOCK_BACKGROUNDED",
            ErrorKind::Timeout => "ERR_WAKE_LOCK_TIMEOUT",
            ErrorKind::NotAcquired => "ERR_WAKE_LOCK_NOT_ACQUIRED",
            ErrorKind::Backend => "ERR_WAKE_LOCK_BACKEND",
        }
    }

    /// What to do about it, in one sentence.
    pub fn message(self) -> &'static str {
        match self {
            ErrorKind::NotSupported => "Wake locks are not supported on this platform or browser.",
            ErrorKind::InvalidArgument => {
                "The options do not request any lock: set display, idle or sleep."
            }
            ErrorKind::NoActivity => {
                "There is no window or activity to hold the lock on right now."
            }
            ErrorKind::Backgrounded => {
                "The app is backgrounded, so the idle-timer change does not apply now."
            }
            ErrorKind::Timeout => "The platform took too long to answer. Try again.",
            ErrorKind::NotAcquired => {
                "That lock is not held: it was released already or never acquired."
            }
            ErrorKind::Backend => "The platform's wake lock service failed.",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_codes_and_messages_are_stable() {
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
            assert_eq!(kind.code(), code);
            assert_eq!(kind.to_string(), code);
            assert_eq!(kind.message(), message);
            assert_eq!(WakeLockError::new(kind).code(), code);
        }
    }
}
