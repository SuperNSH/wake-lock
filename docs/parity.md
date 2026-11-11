# Parity with `wakelock_plus`, `expo-keep-awake` and `keepawake-rs`

Each row names one API of wakelock_plus 1.8.2 (platform interface 1.7.0), expo-keep-awake 58.0.3 or keepawake-rs 0.4.3 — the names come from `.refs/wakelock_plus/`, `.refs/expo/packages/expo-keep-awake/src/` and `.refs/keepawake-rs/src/`, read on 2026-11-09 — the Rust and TypeScript items that answer it here, the status, and the test that covers it.

**Status of this document: verified-pending-CI.** AGENTS.md: a claim with no passing run is unverified. This change set carries real bridges on all five platforms, compile-verified only off-host; no bridge has run on its device, and no CI run has executed the tests named below, so every row — each Done included — is a claim, not an observation. When a `master` run passes them, delete this paragraph.

Status words:

- **Done**: works on every platform the row names, as the reference does.
- **Partial**: works, with a limit or a deliberate difference named in the row.
- **Accepted**: the name is accepted and passed on, but the platform does nothing with it.
- **Platform only**: the effect exists on a device and no CI runner can show it; the test checks what the library passes on.
- **Not provided**: absent in v0; the row names the workaround.

The Rust items live in `crates/nsh-wake-lock/src/` (`options.rs`, `lock.rs`, `fake.rs`, `platform/`), the TypeScript items in `packages/nsh-wake-lock/src/` (`options.ts`, `lock.ts`, `tags.ts`, `web.ts`, `memory.ts`). Covered by names the test that covers the row — Rust tests in `crates/nsh-wake-lock/tests/`, TypeScript tests in `packages/nsh-wake-lock/tests/`; the tests land with this change set and none is in the tree yet. An unqualified test name exists in both languages.

## `WakelockPlus`

| `wakelock_plus` | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `WakelockPlus.enable()` | `WakeLock::acquire` with `display: true`, the `WakeLockGuard` held | `enable()` in `src/tags.ts` | Done. One module-level display lock; a redundant enable is absorbed, as `wakelock_plus` does. Rust has no module-level lock: each `acquire` returns its own guard | `enable_disable_and_toggle_are_idempotent` (`tests/tags.test.ts`) |
| `WakelockPlus.disable()` | `WakeLockGuard::release(mut self)`, or drop | `disable()` in `src/tags.ts` | Done. A redundant disable is absorbed. A released guard disarms, so a second release is a no-op, never a backend call | `enable_disable_and_toggle_are_idempotent` (`tests/tags.test.ts`); `explicit_release_releases_exactly_once` |
| `WakelockPlus.toggle(enable:)` | `acquire` or `release` per the flag | `toggle(enable)` in `src/tags.ts` | Done. Both arms idempotent over the module lock | `enable_disable_and_toggle_are_idempotent` (`tests/tags.test.ts`) |
| `WakelockPlus.enabled` (getter) | `WakeLock::is_active()` | `WakeLockBackend.isActive()`; `src/tags.ts` exports no `enabled` getter | Partial. Reports whether this library holds a lock, which is what the plugin asks its own platform side. The tag surface has no query; callers track their own activate/deactivate pairs | `is_active_reports_whether_a_lock_is_held` |

## `expo-keep-awake`

| `expo-keep-awake` | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `ExpoKeepAwakeTag` (`'ExpoKeepAwakeDefaultTag'`) | not in the API; tags live in TypeScript | `ExpoKeepAwakeTag` in `src/tags.ts`, same value | Done. Same constant, same role as the default tag | `activate_then_deactivate_per_tag` (`tests/tags.test.ts`) |
| `isAvailableAsync()` | `WakeLock::is_supported()` | `isAvailableAsync()` in `src/tags.ts` over `webBackend()` | Done. Web reports exactly `'wakeLock' in navigator`, as expo's web module does; elsewhere the backend answers | `unsupported_platform_reports_not_supported`; `is_available_async_reports_false_without_navigator_wake_lock` (`tests/web.test.ts`) |
| `useKeepAwake(tag?, options?)` | — | not provided in v0 | Not provided. No React peer dependency in v0. Workaround: `activateKeepAwakeAsync(tag)` on mount and `deactivateKeepAwake(tag)` in the cleanup — the hook's effect is exactly that pair | none |
| `activateKeepAwake(tag)` (deprecated) | — | not provided; only the async name exists | Not provided. Workaround: call `activateKeepAwakeAsync` — expo's own deprecation target, same behavior without the `console.warn` | `activate_then_deactivate_per_tag` (`tests/tags.test.ts`) |
| `activateKeepAwakeAsync(tag?)` | `WakeLock::acquire` with `display: true`, one guard per call | `activateKeepAwakeAsync(tag?)` in `src/tags.ts` | Done. One display lock per tag; re-activating a tag releases the old lock first. Several tags hold several locks, as expo does | `activate_then_deactivate_per_tag`; `reactivating_a_tag_replaces_one_lock` (`tests/tags.test.ts`) |
| `deactivateKeepAwake(tag?)` | `WakeLockGuard::release`, or drop | `deactivateKeepAwake(tag?)` in `src/tags.ts` | Partial. An unknown tag rejects `ERR_WAKE_LOCK_NOT_ACQUIRED` — a deliberate difference: expo's native side silently no-ops an unknown tag and its web side throws `ERR_KEEP_AWAKE_TAG_INVALID` | `deactivate_of_unknown_tag_reports_not_acquired` (`tests/tags.test.ts`); `unknown_handle_release_reports_not_acquired` |
| `addListener(tagOrListener, listener?)` | — | not provided in v0 | Not provided. Workaround: the only event is the web sentinel's `release`, fired when the page hides — re-acquire on `document.visibilitychange` | none |
| `KeepAwakeOptions` (`suppressDeactivateWarnings`, `listener`) | — | not provided in v0 | Not provided. Workaround for `suppressDeactivateWarnings`: `.catch(() => {})` on `deactivateKeepAwake`, which is what the option does inside expo's hook cleanup. `listener` is covered by the `addListener` row | none |
| `KeepAwakeEvent`, `KeepAwakeListener` | — | not provided in v0 | Not provided. They exist only to serve `addListener` | none |

## `keepawake::Builder` and `AwakeHandle`

| `keepawake-rs` | Rust | TypeScript | Status | Covered by |
|---|---|---|---|---|
| `Builder::display(bool)` | `WakeLockOptions::display` | `display` | Partial. macOS a `PreventUserIdleDisplaySleep` assertion, Windows a display `PowerSetRequest`, Linux an `org.freedesktop.ScreenSaver` inhibit, web the `"screen"` sentinel. Android `FLAG_KEEP_SCREEN_ON` lands on the activity window current at acquire and release and is never re-applied when the activity is recreated; wakelock_plus re-applies on activity attach | `options_reach_the_backend`; the device halves are Platform only |
| `Builder::idle(bool)` | `WakeLockOptions::idle` | `idle` | Done on macOS (`PreventUserIdleSystemSleep`), Windows (system request) and Linux (a logind `login1` inhibit on the system bus, as keepawake-rs takes it). Accepted on iOS, Android and web: no separate idle channel there, display is the only lock | `options_reach_the_backend` |
| `Builder::sleep(bool)` | `WakeLockOptions::sleep` | `sleep` | Done on macOS (`PreventSystemSleep`), Windows (away-mode request; a no-op under modern standby) and Linux (a logind `login1` inhibit on the system bus, as keepawake-rs takes it). Accepted on iOS, Android and web | `options_reach_the_backend` |
| `Builder::reason(impl Into<String>)` | `WakeLockOptions::reason: Option<String>` | `reason` | Done. Bridges default to `"User requested"`, as keepawake-rs. Used on macOS (assertion name), Windows (part of the power request's reason string) and Linux (inhibit reason); accepted on iOS, Android and web | `options_reach_the_backend` |
| `Builder::app_name(impl Into<String>)` | `WakeLockOptions::app_name: Option<String>` | `appName` | Done. Default `"nsh-wake-lock"` where keepawake-rs defaults to `"keepawake-rs"`. Used on Linux (the logind `who`) and Windows (part of the reason string); accepted on macOS, iOS, Android and web | `options_reach_the_backend` |
| `Builder::app_reverse_domain(impl Into<String>)` | `WakeLockOptions::app_reverse_domain: Option<String>` | `appReverseDomain` | Done. Default `"io.github.supernsh.wake-lock"` where keepawake-rs defaults to `"io.github.segevfiner.keepawake-rs"`. Used on Linux as the ScreenSaver inhibit's application name and on Windows (part of the reason string); accepted on macOS, iOS, Android and web | `options_reach_the_backend` |
| `Builder::create() -> Result<AwakeHandle>` | `WakeLock::acquire(&options) -> Result<WakeLockGuard, WakeLockError>` | `WakeLock.acquire(options)`, which returns a `WakeLockGuard` | Partial. The guard releases on drop exactly as `AwakeHandle`, and still releases through a panic. Difference: all-false options reject `ERR_WAKE_LOCK_INVALID_ARGUMENT`; keepawake-rs would create an empty handle | `all_false_options_are_invalid_argument`; `drop_releases_the_lock`; `unwind_releases_the_lock` |
| `AwakeHandle::set_display(bool)` (macOS only) | not in the API | not in the API | Not provided. Workaround: release the guard and acquire another with `display` changed; a release resolves exactly once, so the swap cannot double-hold | none beyond `explicit_release_releases_exactly_once` and `options_reach_the_backend` |

## Error mapping

| Reference failure | Code here | Status | Covered by |
|---|---|---|---|
| expo `UnavailabilityError('ExpoKeepAwake', 'addListenerForTag')`: the native module lacks a method | `ERR_WAKE_LOCK_NOT_SUPPORTED` | Done. Ours ships no optional native methods; the unsupported case is a browser without `navigator.wakeLock` or an unsupported target, reported up front | `unsupported_platform_reports_not_supported`; `is_available_async_reports_false_without_navigator_wake_lock` (`tests/web.test.ts`) |
| expo web `CodedError('ERR_KEEP_AWAKE_TAG_INVALID')`: `deactivate` of a tag that never activated | `ERR_WAKE_LOCK_NOT_ACQUIRED` | Done, and extended to every platform: expo's native `deactivate` silently no-ops an unknown tag | `deactivate_of_unknown_tag_reports_not_acquired` (`tests/tags.test.ts`); `unknown_handle_release_reports_not_acquired` |
| Android: the original activity dead or deactivated; expo's call rejects unhandled, which is why `suppressDeactivateWarnings` exists | `ERR_WAKE_LOCK_NO_ACTIVITY` | Done. Ours rejects with the code instead of an unhandled rejection | `missing_activity_reports_no_activity` |
| iOS: an idle-timer change while backgrounded; the system silently does not apply it | `ERR_WAKE_LOCK_BACKGROUNDED` | Done in the library; the device half is Platform only | `backgrounded_reports_backgrounded` |
| keepawake-rs macOS: a nonzero `IOReturn` from `IOPMAssertionCreateWithName`, wrapped untyped in anyhow's `IO error: {:#x}` | `ERR_WAKE_LOCK_BACKEND`, `status` carrying the IOReturn | Done. `status` carries the IOReturn; a real one needs a device | `every_kind_carries_its_stable_code`; a real IOReturn is Platform only |
| keepawake-rs Linux: zbus connection, proxy or call failures, propagated untyped through anyhow | `ERR_WAKE_LOCK_BACKEND`, no `status`: zbus gives no number. An answer slower than 5 s resolves `ERR_WAKE_LOCK_TIMEOUT` instead of hanging | Done | `scripted_backend_error_surfaces_with_code` |
| keepawake-rs Windows: `SetThreadExecutionState` returning 0, turned into `WindowsError::from_win32()` | `ERR_WAKE_LOCK_BACKEND`, `status` carrying the Win32 last error | Done. Ours uses `PowerCreateRequest`/`PowerSetRequest`, which fail through the same last error | `scripted_backend_error_surfaces_with_code` |

Two codes are ours alone. `ERR_WAKE_LOCK_INVALID_ARGUMENT`: options with `display`, `idle` and `sleep` all false (`all_false_options_are_invalid_argument`). `ERR_WAKE_LOCK_TIMEOUT`: the Linux bridge's 5-second D-Bus bound and the web bridge's 10-second request deadline (`REQUEST_TIMEOUT_MS` in `platform/web.rs`), where keepawake-rs's blocking call would wait forever.
