import { afterEach, expect, test, vi } from "vitest";
import { isAvailableAsync, webBackend } from "../src/index";

afterEach(() => {
  vi.unstubAllGlobals();
});

test("is_available_async_reports_false_without_navigator_wake_lock", async () => {
  await expect(isAvailableAsync()).resolves.toBe(false);
  await expect(webBackend().isSupported()).resolves.toBe(false);
});

test("web_sentinel_release_flips_is_active", async () => {
  const sentinel = {
    released: false,
    releaseCalls: 0,
    async release(): Promise<void> {
      sentinel.releaseCalls += 1;
      sentinel.released = true;
    },
  };
  vi.stubGlobal("navigator", { wakeLock: { request: async () => sentinel } });

  const backend = webBackend();
  const handle = await backend.acquire({ display: true });
  await expect(backend.isActive()).resolves.toBe(true);

  // The browser revokes the lock (page hidden); src/web.ts observes it via `released`.
  sentinel.released = true;
  await expect(backend.isActive()).resolves.toBe(false);
  expect(sentinel.releaseCalls).toBe(0);

  // The bridge skips sentinel.release() for an already-released lock: no double release.
  backend.release(handle);
  expect(sentinel.releaseCalls).toBe(0);
});
