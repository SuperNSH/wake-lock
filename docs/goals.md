# Goals

Keep the screen or the machine awake while an NSH app needs it, and release the lock reliably: on scope exit, on panic unwind, and on explicit release, in that order of guarantees. One error model, identical in Rust and TypeScript.

Two deliverables carry the same behavior:

- `crates/nsh-wake-lock`: the Rust crate — options, error model, `WakeLock`/`WakeLockGuard` with release on drop, the `Backend` trait, a fake backend for tests, and platform bridges (macOS, iOS, Android, Windows, Linux, web).
- `packages/nsh-wake-lock`: the TypeScript package — the same error model and lock, expo-style keep-awake tags, a feature-detecting web backend, and a memory backend for tests.

Behavior parity targets, cloned read-only under `.refs/` (see [decisions.md](decisions.md)): Flutter `wakelock_plus`, React Native `expo-keep-awake`, Rust `keepawake-rs`.

## Acceptance criteria

- **Resolve exactly once.** Every operation ends in success, a coded failure, or a cancel. Nothing hangs. The Linux bridge wraps its D-Bus calls in a 5 s timeout that resolves `ERR_WAKE_LOCK_TIMEOUT`.
- **Never panic in a release path.** `WakeLockGuard`'s `Drop` releases while armed and ignores the result — keepawake-rs's Linux `Drop` `unwrap()`s its uninhibit and we do not, because a release failure during unwind must not abort the unwind. Explicit `release()` surfaces the error instead of swallowing it.
- **Coded errors only.** The seven stable codes below, with identical strings in Rust and TypeScript; a platform's own status code rides along in `status` where it gives one.
- **Both languages tested.** Every edge case in the tables below has its named test in this change set's Rust and TypeScript suites, and a test counts only after it has been seen to fail once.
- **Gates.** CI runs, per the spec: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets`; `npm run typecheck` (`tsc --noEmit`); `npm test` (`vitest run --passWithNoTests`). Each platform bridge is compile-verified for its target triple (apple, android, windows, linux, wasm32) with `cargo clippy` on the lib into a separate target dir. No report may claim bridge runtime behavior a runner did not show.

## Error kinds

`WakeLockError` carries the kind plus an optional `status: Option<i64>` holding the platform's own code where it gives one; `code()` returns the stable string. TypeScript mirrors this: `class WakeLockError extends Error` with a readonly `code` field, plus `wakeLockError(code)` and `asWakeLockError(unknown)`, which maps anything unrecognized to `ERR_WAKE_LOCK_BACKEND`.

| Kind | Code | Message |
|---|---|---|
| NotSupported | `ERR_WAKE_LOCK_NOT_SUPPORTED` | Wake locks are not supported on this platform or browser. |
| InvalidArgument | `ERR_WAKE_LOCK_INVALID_ARGUMENT` | The options do not request any lock: set display, idle or sleep. |
| NoActivity | `ERR_WAKE_LOCK_NO_ACTIVITY` | There is no window or activity to hold the lock on right now. |
| Backgrounded | `ERR_WAKE_LOCK_BACKGROUNDED` | The app is backgrounded, so the idle-timer change does not apply now. |
| Timeout | `ERR_WAKE_LOCK_TIMEOUT` | The platform took too long to answer. Try again. |
| NotAcquired | `ERR_WAKE_LOCK_NOT_ACQUIRED` | That lock is not held: it was released already or never acquired. |
| Backend | `ERR_WAKE_LOCK_BACKEND` | The platform's wake lock service failed. |

## Edge cases

Cases 1-12 are the predesigned list; the test names are contract. The spec asks for each in both languages; JavaScript has no deterministic destruction and no unwinding, so cases 1 and 2 are exercised in Rust, and the TypeScript guard's release-exactly-once-then-documented-no-op (case 3) carries the same guarantee there.

| # | Case | Named test | Suite |
|---|---|---|---|
| 1 | A dropped guard releases exactly once (the brief's crash-scope case) | `drop_releases_the_lock` | Rust |
| 2 | A panic unwinding through a scope still releases (`catch_unwind`) | `unwind_releases_the_lock` | Rust |
| 3 | Explicit release then drop touches the backend once | `explicit_release_releases_exactly_once` | Rust + TS |
| 4 | All-false options are refused (the brief's battery case) | `all_false_options_are_invalid_argument` | Rust + TS |
| 5 | Every kind carries its stable code | `every_kind_carries_its_stable_code` | Rust + TS |
| 6 | A platform or browser without wake locks reports NotSupported | `unsupported_platform_reports_not_supported` | Rust + TS |
| 7 | Acquire while backgrounded reports Backgrounded (the brief's iOS case) | `backgrounded_reports_backgrounded` | Rust + TS |
| 8 | No window or activity reports NoActivity (the brief's Android case) | `missing_activity_reports_no_activity` | Rust + TS |
| 9 | Release of a stale or double-released handle reports NotAcquired | `unknown_handle_release_reports_not_acquired` | Rust + TS |
| 10 | Flags and reason/app_name/app_reverse_domain reach the backend | `options_reach_the_backend` | Rust + TS |
| 11 | `is_active` reports whether a lock is held (wakelock_plus `enabled` parity) | `is_active_reports_whether_a_lock_is_held` | Rust + TS |
| 12 | A failing backend surfaces Backend with its code and status | `scripted_backend_error_surfaces_with_code` | Rust + TS |

TypeScript suite additions, as specified: `tests/tags.test.ts` with `activate_then_deactivate_per_tag`, `reactivating_a_tag_replaces_one_lock`, `deactivate_of_unknown_tag_reports_not_acquired`, `enable_disable_and_toggle_are_idempotent`; `tests/web.test.ts` with `is_available_async_reports_false_without_navigator_wake_lock`.

### Added after reading the references

Note for the test areas, via this document: cases 13 and 14 below are additions this document decided after reading the reference sources; the Rust and TypeScript suites must include them.

| # | Case | Named test | Suite | Reference evidence |
|---|---|---|---|---|
| 13 | Multiple locks coexist; releasing one leaves the others held | `multiple_locks_coexist_until_each_releases` | Rust + TS | expo keeps a tag set and clears the flag only when the last tag deactivates (`ExpoKeepAwakeManager.kt`); keepawake-rs holds one cookie or fd per lock. The predesigned spec allows multiple guards with no single-flight gate but named no test for it. |
| 14 | A browser-initiated sentinel release flips `is_active` to false | `web_sentinel_release_flips_is_active` | TS (`tests/web.test.ts`) | The browser releases a screen wake lock when the page hides; expo's web module tracks the sentinel's `release` event. The web backend must observe it so `is_active` does not lie. |

### Platform behavior the references require, recorded for the bridge areas

These are runtime platform behaviors the references handle that CI cannot exercise; the apple and web bridge areas were told via this document. Both are compile-verified only and their runtime behavior stays unverified until a device or browser run shows it.

- iOS re-applies the idle timer on foreground: expo re-asserts `idleTimerDisabled` in `appWillEnterForeground` when tags are still active (`KeepAwakeModule.swift`), because the system clears it in the background. The apple bridge must do the same for live guards.
- The web backend drops its tracking when the sentinel reports release, so a released-but-unqueried lock does not count as held (case 14 is its CI-verifiable half).

### Coverage audit against the references

- wakelock_plus: `toggle` idempotency (TS tags idempotency test), the `enabled` query (case 11), web via a wake-lock shim (web backend, cases 6 and 14).
- expo-keep-awake: default tag and per-tag activate/deactivate (tags tests), unknown-tag error (case 9 and `deactivate_of_unknown_tag_reports_not_acquired`), foreground re-apply (platform note above), web availability check (case 6 and `is_available_async_reports_false_without_navigator_wake_lock`), `addListener` and `useKeepAwake` (deferred; see [decisions.md](decisions.md)).
- keepawake-rs: release on drop (cases 1-3), option defaults (case 10), one assertion/cookie/fd per lock (case 13), Linux `Drop` panic (never-panic criterion), slow D-Bus (Timeout kind).
