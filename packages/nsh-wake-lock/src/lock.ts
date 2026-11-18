import type { LockHandle, WakeLockBackend } from "./backend";
import { asWakeLockError, wakeLockError } from "./errors";
import type { WakeLockOptions } from "./options";

/** Releases exactly once; later calls are documented no-ops. */
export class WakeLockGuard {
  private released = false;

  constructor(
    private readonly backend: WakeLockBackend,
    private readonly handle: LockHandle,
  ) {}

  release(): void {
    if (this.released) return;
    this.released = true;
    this.backend.release(this.handle);
  }
}

export class WakeLock {
  constructor(private readonly backend: WakeLockBackend) {}

  isSupported(): Promise<boolean> {
    return this.backend.isSupported();
  }

  async acquire(options: WakeLockOptions): Promise<WakeLockGuard> {
    if (!options.display && !options.idle && !options.sleep) {
      throw wakeLockError("ERR_WAKE_LOCK_INVALID_ARGUMENT");
    }
    try {
      const handle = await this.backend.acquire(options);
      return new WakeLockGuard(this.backend, handle);
    } catch (error) {
      throw asWakeLockError(error);
    }
  }

  isActive(): Promise<boolean> {
    return this.backend.isActive();
  }
}
