import { expect, test } from "vitest";
import {
  activateKeepAwakeAsync,
  deactivateKeepAwake,
  disable,
  enable,
  setTagBackend,
  toggle,
  WakeLockError,
} from "../src/index";
import { memoryBackend } from "../src/memory";

test("activate_then_deactivate_per_tag", async () => {
  const backend = memoryBackend();
  setTagBackend(backend);
  await activateKeepAwakeAsync();
  await expect(backend.isActive()).resolves.toBe(true);
  await deactivateKeepAwake();
  await expect(backend.isActive()).resolves.toBe(false);

  await activateKeepAwakeAsync("audio");
  await activateKeepAwakeAsync("video");
  expect(backend.acquires()).toBe(3);
  await expect(backend.isActive()).resolves.toBe(true);
  await deactivateKeepAwake("audio");
  await expect(backend.isActive()).resolves.toBe(true);
  await deactivateKeepAwake("video");
  await expect(backend.isActive()).resolves.toBe(false);
  expect(backend.releases()).toBe(3);
});

test("reactivating_a_tag_replaces_one_lock", async () => {
  const backend = memoryBackend();
  setTagBackend(backend);
  await activateKeepAwakeAsync("video");
  await activateKeepAwakeAsync("video");
  expect(backend.acquires()).toBe(2);
  expect(backend.releases()).toBe(1);
  await expect(backend.isActive()).resolves.toBe(true);
  await deactivateKeepAwake("video");
  expect(backend.releases()).toBe(2);
  await expect(backend.isActive()).resolves.toBe(false);
});

test("deactivate_of_unknown_tag_reports_not_acquired", async () => {
  const backend = memoryBackend();
  setTagBackend(backend);
  const rejection = deactivateKeepAwake("never-activated");
  await expect(rejection).rejects.toBeInstanceOf(WakeLockError);
  await expect(rejection).rejects.toMatchObject({ code: "ERR_WAKE_LOCK_NOT_ACQUIRED" });
  expect(backend.releases()).toBe(0);
});

test("enable_disable_and_toggle_are_idempotent", async () => {
  const backend = memoryBackend();
  setTagBackend(backend);
  await enable();
  await enable();
  expect(backend.acquires()).toBe(1);
  await expect(backend.isActive()).resolves.toBe(true);
  await disable();
  await disable();
  expect(backend.releases()).toBe(1);
  await expect(backend.isActive()).resolves.toBe(false);
  await toggle(true);
  await toggle(true);
  expect(backend.acquires()).toBe(2);
  await toggle(false);
  await toggle(false);
  expect(backend.releases()).toBe(2);
  await expect(backend.isActive()).resolves.toBe(false);
});
