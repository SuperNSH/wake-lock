import type { WakeLockBackend } from "./backend";
import { asWakeLockError, wakeLockError } from "./errors";
import { WakeLock, WakeLockGuard } from "./lock";
import { webBackend } from "./web";

export const ExpoKeepAwakeTag = "ExpoKeepAwakeDefaultTag";

let backend: WakeLockBackend = webBackend();
const taggedLocks = new Map<string, WakeLockGuard>();
// Same-tag activates and deactivates queue here: a slow acquire is never orphaned and a
// concurrent deactivate sees the landed guard instead of a timing-dependent map state.
const tagQueue = new Map<string, Promise<void>>();
let moduleLock: WakeLockGuard | null = null;
// Concurrent toggle() calls queue here so two enable()s cannot leak the first guard.
let moduleQueue: Promise<void> | null = null;

/** Swaps the backend used by every function in this module; call it before acquiring. */
export function setTagBackend(next: WakeLockBackend): void {
  backend = next;
}

export function isAvailableAsync(): Promise<boolean> {
  return backend.isSupported();
}

export async function activateKeepAwakeAsync(tag: string = ExpoKeepAwakeTag): Promise<void> {
  const run = (tagQueue.get(tag) ?? Promise.resolve())
    .catch(() => undefined)
    .then(() => acquireTag(tag));
  tagQueue.set(tag, run);
  try {
    await run;
  } finally {
    if (tagQueue.get(tag) === run) tagQueue.delete(tag);
  }
}

async function acquireTag(tag: string): Promise<void> {
  // Release the old lock first so a tag never owns two locks at once.
  taggedLocks.get(tag)?.release();
  taggedLocks.delete(tag);
  try {
    const guard = await new WakeLock(backend).acquire({ display: true });
    taggedLocks.set(tag, guard);
  } catch (error) {
    throw asWakeLockError(error);
  }
}

/** Unknown tags reject with ERR_WAKE_LOCK_NOT_ACQUIRED instead of expo's console.warn (see docs/parity.md). */
export async function deactivateKeepAwake(tag: string = ExpoKeepAwakeTag): Promise<void> {
  const run = (tagQueue.get(tag) ?? Promise.resolve())
    .catch(() => undefined)
    .then(() => releaseTag(tag));
  tagQueue.set(tag, run);
  try {
    await run;
  } finally {
    if (tagQueue.get(tag) === run) tagQueue.delete(tag);
  }
}

function releaseTag(tag: string): void {
  const guard = taggedLocks.get(tag);
  if (guard === undefined) throw wakeLockError("ERR_WAKE_LOCK_NOT_ACQUIRED");
  guard.release();
  taggedLocks.delete(tag);
}

/** wakelock_plus parity: one module-level lock; enable and disable are idempotent. */
export async function toggle(enable: boolean): Promise<void> {
  const run = (moduleQueue ?? Promise.resolve())
    .catch(() => undefined)
    .then(() => setModuleLock(enable));
  moduleQueue = run;
  try {
    await run;
  } finally {
    if (moduleQueue === run) moduleQueue = null;
  }
}

async function setModuleLock(enable: boolean): Promise<void> {
  if (enable) {
    if (moduleLock !== null) return;
    try {
      moduleLock = await new WakeLock(backend).acquire({ display: true });
    } catch (error) {
      throw asWakeLockError(error);
    }
    return;
  }
  moduleLock?.release();
  moduleLock = null;
}

export function enable(): Promise<void> {
  return toggle(true);
}

export function disable(): Promise<void> {
  return toggle(false);
}
