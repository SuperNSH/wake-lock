import type { LockHandle, WakeLockBackend } from "./backend";
import { asWakeLockError, wakeLockError } from "./errors";
import type { WakeLockOptions } from "./options";

/** Minimal sentinel shape; lib.dom's WakeLockSentinel is not on every TS lib version we support. */
interface SentinelLike {
  release(): Promise<void>;
  readonly released: boolean;
}

interface WakeLockLike {
  request(type: "screen"): Promise<SentinelLike>;
}

function wakeLockApi(): WakeLockLike | undefined {
  if (typeof navigator === "undefined") return undefined;
  return (navigator as { wakeLock?: WakeLockLike }).wakeLock;
}

/** Web Screen Wake Lock backend; the only lock type the API offers is "screen". */
export function webBackend(): WakeLockBackend {
  const sentinels = new Map<number, SentinelLike>();
  let nextId = 1;

  return {
    async isSupported(): Promise<boolean> {
      return wakeLockApi() !== undefined;
    },

    async acquire(options: WakeLockOptions): Promise<LockHandle> {
      const api = wakeLockApi();
      if (api === undefined || !options.display) {
        throw wakeLockError("ERR_WAKE_LOCK_NOT_SUPPORTED");
      }
      try {
        const sentinel = await api.request("screen");
        const handle: LockHandle = { id: nextId++ };
        sentinels.set(handle.id, sentinel);
        return handle;
      } catch (error) {
        throw asWakeLockError(error);
      }
    },

    release(handle: LockHandle): void {
      const sentinel = sentinels.get(handle.id);
      if (sentinel === undefined) throw wakeLockError("ERR_WAKE_LOCK_NOT_ACQUIRED");
      sentinels.delete(handle.id);
      // sentinel.release() rejects only if already released; the sync contract ignores that.
      if (!sentinel.released) void sentinel.release().catch(() => undefined);
    },

    async isActive(): Promise<boolean> {
      for (const sentinel of sentinels.values()) {
        if (!sentinel.released) return true;
      }
      return false;
    },
  };
}
