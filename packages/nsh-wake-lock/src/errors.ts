/** Codes and messages mirror the Rust crate's ErrorKind exactly; wording is part of the contract. */
export type WakeLockErrorCode =
  | "ERR_WAKE_LOCK_NOT_SUPPORTED"
  | "ERR_WAKE_LOCK_INVALID_ARGUMENT"
  | "ERR_WAKE_LOCK_NO_ACTIVITY"
  | "ERR_WAKE_LOCK_BACKGROUNDED"
  | "ERR_WAKE_LOCK_TIMEOUT"
  | "ERR_WAKE_LOCK_NOT_ACQUIRED"
  | "ERR_WAKE_LOCK_BACKEND";

export const MESSAGES: Readonly<Record<WakeLockErrorCode, string>> = {
  ERR_WAKE_LOCK_NOT_SUPPORTED: "Wake locks are not supported on this platform or browser.",
  ERR_WAKE_LOCK_INVALID_ARGUMENT: "The options do not request any lock: set display, idle or sleep.",
  ERR_WAKE_LOCK_NO_ACTIVITY: "There is no window or activity to hold the lock on right now.",
  ERR_WAKE_LOCK_BACKGROUNDED: "The app is backgrounded, so the idle-timer change does not apply now.",
  ERR_WAKE_LOCK_TIMEOUT: "The platform took too long to answer. Try again.",
  ERR_WAKE_LOCK_NOT_ACQUIRED: "That lock is not held: it was released already or never acquired.",
  ERR_WAKE_LOCK_BACKEND: "The platform's wake lock service failed.",
};

export class WakeLockError extends Error {
  readonly code: WakeLockErrorCode;

  constructor(code: WakeLockErrorCode, message: string = MESSAGES[code]) {
    super(message);
    this.name = "WakeLockError";
    this.code = code;
  }
}

export function wakeLockError(code: WakeLockErrorCode): WakeLockError {
  return new WakeLockError(code);
}

export function asWakeLockError(value: unknown): WakeLockError {
  return value instanceof WakeLockError ? value : wakeLockError("ERR_WAKE_LOCK_BACKEND");
}
