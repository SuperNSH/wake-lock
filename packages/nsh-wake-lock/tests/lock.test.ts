import { expect, test } from "vitest";
import {
  activateKeepAwakeAsync,
  asWakeLockError,
  MESSAGES,
  setTagBackend,
  WakeLock,
  WakeLockError,
  wakeLockError,
} from "../src/index";
import type { WakeLockErrorCode } from "../src/index";
import { memoryBackend } from "../src/memory";

test("explicit_release_releases_exactly_once", async () => {
  const backend = memoryBackend();
  const guard = await new WakeLock(backend).acquire({ display: true });
  guard.release();
  guard.release();
  expect(backend.releases()).toBe(1);
  expect(backend.acquires()).toBe(1);
  await expect(backend.isActive()).resolves.toBe(false);
});

test("all_false_options_are_invalid_argument", async () => {
  const backend = memoryBackend();
  const lock = new WakeLock(backend);
  for (const options of [{ display: false, idle: false, sleep: false }, {}]) {
    const rejection = lock.acquire(options);
    await expect(rejection).rejects.toBeInstanceOf(WakeLockError);
    await expect(rejection).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_INVALID_ARGUMENT" });
  }
  expect(backend.acquires()).toBe(0);
  expect(backend.lastOptions()).toBeUndefined();
});

test("every_kind_carries_its_stable_code", () => {
  const stable: Array<[WakeLockErrorCode, string]> = [
    ["ERR_WAKE_LOCK_NOT_SUPPORTED", "Wake locks are not supported on this platform or browser."],
    ["ERR_WAKE_LOCK_INVALID_ARGUMENT", "The options do not request any lock: set display, idle or sleep."],
    ["ERR_WAKE_LOCK_NO_ACTIVITY", "There is no window or activity to hold the lock on right now."],
    ["ERR_WAKE_LOCK_BACKGROUNDED", "The app is backgrounded, so the idle-timer change does not apply now."],
    ["ERR_WAKE_LOCK_TIMEOUT", "The platform took too long to answer. Try again."],
    ["ERR_WAKE_LOCK_NOT_ACQUIRED", "That lock is not held: it was released already or never acquired."],
    ["ERR_WAKE_LOCK_BACKEND", "The platform's wake lock service failed."],
  ];
  for (const [code, message] of stable) {
    const error = wakeLockError(code);
    expect(error).toBeInstanceOf(WakeLockError);
    expect(error.code).toBe(code);
    expect(error.message).toBe(message);
    expect(MESSAGES[code]).toBe(message);
    expect(asWakeLockError(error)).toBe(error);
  }
  for (const unrecognized of [new Error("boom"), "noise", 42, undefined]) {
    const wrapped = asWakeLockError(unrecognized);
    expect(wrapped).toBeInstanceOf(WakeLockError);
    expect(wrapped.code).toBe("ERR_WAKE_LOCK_BACKEND");
    expect(wrapped.message).toBe(MESSAGES.ERR_WAKE_LOCK_BACKEND);
  }
});

test("unsupported_platform_reports_not_supported", async () => {
  const backend = memoryBackend({ supported: false });
  await expect(new WakeLock(backend).isSupported()).resolves.toBe(false);
});

test("backgrounded_reports_backgrounded", async () => {
  const backend = memoryBackend({ acquireAnswers: [wakeLockError("ERR_WAKE_LOCK_BACKGROUNDED")] });
  const rejection = new WakeLock(backend).acquire({ idle: true });
  await expect(rejection).rejects.toBeInstanceOf(WakeLockError);
  await expect(rejection).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_BACKGROUNDED" });
  expect(backend.acquires()).toBe(0);

  const tags = memoryBackend({ acquireAnswers: [wakeLockError("ERR_WAKE_LOCK_BACKGROUNDED")] });
  setTagBackend(tags);
  const tagged = activateKeepAwakeAsync("backgrounded-tag");
  await expect(tagged).rejects.toBeInstanceOf(WakeLockError);
  await expect(tagged).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_BACKGROUNDED" });
  expect(tags.acquires()).toBe(0);
});

test("missing_activity_reports_no_activity", async () => {
  const backend = memoryBackend({ acquireAnswers: [wakeLockError("ERR_WAKE_LOCK_NO_ACTIVITY")] });
  const rejection = new WakeLock(backend).acquire({ display: true });
  await expect(rejection).rejects.toBeInstanceOf(WakeLockError);
  await expect(rejection).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_NO_ACTIVITY" });
  expect(backend.acquires()).toBe(0);
});

test("unknown_handle_release_reports_not_acquired", async () => {
  const backend = memoryBackend();
  const handle = await backend.acquire({ display: true });
  backend.release(handle);
  for (const stale of [handle, { id: 999 }]) {
    try {
      backend.release(stale);
      expect.unreachable("release of an unknown handle must throw");
    } catch (error) {
      expect(error).toBeInstanceOf(WakeLockError);
      expect((error as WakeLockError).code).toBe("ERR_WAKE_LOCK_NOT_ACQUIRED");
    }
  }
  expect(backend.releases()).toBe(1);
});

test("options_reach_the_backend", async () => {
  const backend = memoryBackend();
  const options = {
    display: true,
    idle: false,
    sleep: true,
    reason: "encoding",
    appName: "NSH Encoder",
    appReverseDomain: "com.nsh.encoder",
  };
  await new WakeLock(backend).acquire(options);
  expect(backend.lastOptions()).toEqual({ ...options });
  expect(backend.acquires()).toBe(1);
});

test("is_active_reports_whether_a_lock_is_held", async () => {
  const backend = memoryBackend();
  const lock = new WakeLock(backend);
  await expect(lock.isActive()).resolves.toBe(false);
  const guard = await lock.acquire({ idle: true });
  await expect(lock.isActive()).resolves.toBe(true);
  guard.release();
  await expect(lock.isActive()).resolves.toBe(false);
});

test("scripted_backend_error_surfaces_with_code", async () => {
  const backend = memoryBackend({
    acquireAnswers: [wakeLockError("ERR_WAKE_LOCK_TIMEOUT"), new Error("plain failure")],
  });
  const lock = new WakeLock(backend);
  const timeout = lock.acquire({ display: true });
  await expect(timeout).rejects.toBeInstanceOf(WakeLockError);
  await expect(timeout).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_TIMEOUT" });
  const wrapped = lock.acquire({ display: true });
  await expect(wrapped).rejects.toBeInstanceOf(WakeLockError);
  await expect(wrapped).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_BACKEND" });
  expect(backend.acquires()).toBe(0);
});

test("multiple_locks_coexist_until_each_releases", async () => {
  const backend = memoryBackend();
  const lock = new WakeLock(backend);
  const first = await lock.acquire({ display: true });
  const second = await lock.acquire({ sleep: true });
  expect(backend.acquires()).toBe(2);
  await expect(lock.isActive()).resolves.toBe(true);
  first.release();
  expect(backend.releases()).toBe(1);
  await expect(lock.isActive()).resolves.toBe(true);
  second.release();
  expect(backend.releases()).toBe(2);
  await expect(lock.isActive()).resolves.toBe(false);
});
