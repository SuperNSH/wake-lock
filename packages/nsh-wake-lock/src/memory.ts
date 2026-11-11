import type { LockHandle, WakeLockBackend } from "./backend";
import { wakeLockError } from "./errors";
import type { WakeLockOptions } from "./options";

export interface MemoryBackendConfig {
  /** Answer for isSupported(); defaults to true. */
  supported?: boolean;
  /** Consumed in order per acquire(): an Error is thrown, a handle is returned as-is. Exhausted: fresh handles. */
  acquireAnswers?: Array<LockHandle | Error>;
  /** Consumed in order per release() of a held handle: the Error is thrown and the lock stays held. */
  releaseFailures?: Error[];
}

export interface MemoryBackend extends WakeLockBackend {
  acquires(): number;
  releases(): number;
  lastOptions(): WakeLockOptions | undefined;
}

/** In-memory backend for tests; counters cover successful operations only. */
export function memoryBackend(config: MemoryBackendConfig = {}): MemoryBackend {
  const supported = config.supported ?? true;
  const acquireAnswers = [...(config.acquireAnswers ?? [])];
  const releaseFailures = [...(config.releaseFailures ?? [])];
  const held = new Map<number, LockHandle>();
  let acquireCount = 0;
  let releaseCount = 0;
  let last: WakeLockOptions | undefined;
  let nextId = 1;

  return {
    async isSupported(): Promise<boolean> {
      return supported;
    },

    async acquire(options: WakeLockOptions): Promise<LockHandle> {
      const scripted = acquireAnswers.shift();
      if (scripted instanceof Error) throw scripted;
      const handle = scripted ?? { id: nextId++ };
      held.set(handle.id, handle);
      acquireCount += 1;
      last = options;
      return handle;
    },

    release(handle: LockHandle): void {
      if (!held.has(handle.id)) throw wakeLockError("ERR_WAKE_LOCK_NOT_ACQUIRED");
      const failure = releaseFailures.shift();
      if (failure !== undefined) throw failure;
      held.delete(handle.id);
      releaseCount += 1;
    },

    async isActive(): Promise<boolean> {
      return held.size > 0;
    },

    acquires(): number {
      return acquireCount;
    },

    releases(): number {
      return releaseCount;
    },

    lastOptions(): WakeLockOptions | undefined {
      return last;
    },
  };
}
