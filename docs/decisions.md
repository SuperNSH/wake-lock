# Decisions

Each entry says what was chosen and why. [goals.md](goals.md) holds the acceptance criteria and edge cases; [parity.md](parity.md) lists what each capability does on each platform.

## Open

None. The scaffold's three open items — NSH version, library choice, platforms in scope — were decided on 2026-11-09 and are recorded below.

## Decided

### A thin own crate on every platform, not a third-party wake crate

The brief's recommended path, adopted. The crate mirrors keepawake-rs's behavior on desktop (macOS `IOPMAssertionCreateWithName`, Windows `PowerCreateRequest`/`PowerSetRequest`/`PowerClearRequest`, Linux zbus `org.freedesktop.ScreenSaver` and `org.gnome.SessionManager` inhibit with the cookie as the lock id), adds the native mobile behavior from wakelock_plus and expo (iOS `idleTimerDisabled`, Android `FLAG_KEEP_SCREEN_ON` through ndk-context and JNI), and wraps the web Screen Wake Lock API. Depending on keepawake-rs directly would import an `anyhow` error surface with no stable codes, no mobile or web backends, and a Linux `Drop` that panics on a failed release; `screen-wake-lock` is young and display-only. An own thin crate keeps the error codes and the guard behavior uniform across platforms and languages. Option semantics and bridge defaults mirror keepawake-rs where the platform wants them: reason defaults to "User requested", app_name to "nsh-wake-lock", app_reverse_domain to "io.github.supernsh.wake-lock".

### The crate does not depend on nsh-core yet

NSH has no wake-lock capability to wrap, so this crate is a new capability in the org's style rather than a binding. NSH native-module registration is deferred: without an nsh-core dependency the local gates stay light (fmt, clippy, `cargo test`, per-triple clippy, `tsc`, vitest), and wiring the crate into NSH apps is a follow-up for when there is a consumer to prove the surface against.

### Platforms in scope: macOS, iOS, Android, Windows, Linux, web

The first release covers all six. Desktop platforms take keepawake-rs semantics with display, idle and sleep locks. Android is display-only through the window flag; iOS uses the idle timer and reports `ERR_WAKE_LOCK_BACKGROUNDED` while backgrounded; web is display-only through the Screen Wake Lock API. Any other target answers `is_supported` with false and `acquire` with `ERR_WAKE_LOCK_NOT_SUPPORTED`.

### expo's `useKeepAwake` hook and `addListener` are not provided in v0

The hook is React-shaped and would force a React peer dependency the package does not otherwise carry; NSH TSX apps are not React. `addListener` needs per-platform release notifications — the browser releases a screen lock when the page hides, iOS clears the idle timer in the background — and v0 surfaces those states through `is_active` polling instead. Both wait for a consumer that needs them.

### References: five clones in `.refs/`, taken 2026-11-09

Read-only; never edited. Canonical repos and pinned commits:

| Clone | Canonical repo | Commit |
|---|---|---|
| `.refs/wakelock_plus` | fluttercommunity/wakelock_plus | `a90c5a7` |
| `.refs/expo` (packages/expo-keep-awake) | expo/expo | `52915f0` |
| `.refs/keepawake-rs` (crate `keepawake` 0.4.3) | rustdesk-org/keepawake-rs | `64d5685` |
| `.refs/nsh` | longbridge/nsh | `091049a` |
| `.refs/biometrics` | SuperNSH/biometrics | `fe622d8` |
