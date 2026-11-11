import type { WakeLockOptions } from "./options";

export interface LockHandle {
  readonly id: number;
}

export interface WakeLockBackend {
  isSupported(): Promise<boolean>;
  acquire(options: WakeLockOptions): Promise<LockHandle>;
  release(handle: LockHandle): void;
  isActive(): Promise<boolean>;
}
