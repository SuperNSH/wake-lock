export { asWakeLockError, MESSAGES, WakeLockError, wakeLockError } from "./errors";
export type { WakeLockErrorCode } from "./errors";
export type { LockHandle, WakeLockBackend } from "./backend";
export { WakeLock, WakeLockGuard } from "./lock";
export type { WakeLockOptions } from "./options";
export {
  activateKeepAwakeAsync,
  deactivateKeepAwake,
  disable,
  enable,
  ExpoKeepAwakeTag,
  isAvailableAsync,
  setTagBackend,
  toggle,
} from "./tags";
export { webBackend } from "./web";
