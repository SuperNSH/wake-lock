# nsh-wake-lock

Keep the screen or the machine awake while the app needs it, and release it reliably.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/wake-lock/.

## Install

```sh
cargo add nsh-wake-lock
```

## Status

**Implemented; Rust and TypeScript tests and per-target compile checks verified by CI.** CI run [43053190833](https://github.com/SuperNSH/wake-lock/actions/runs/43053190833) on `master` (commit 7fb7a61) passed every job: `rust` (fmt, clippy `-D warnings`, `cargo test --all-targets`), `typescript` (typecheck, vitest) and the lib-only clippy checks for aarch64-apple-ios-sim, x86_64-pc-windows-gnu, x86_64-unknown-linux-gnu, wasm32-unknown-unknown and aarch64-linux-android. Red-first: [run 44401086183](https://github.com/SuperNSH/wake-lock/actions/runs/44401086183) broke Drop's release, the TS release-once guard and the TS argument check on purpose, and `drop_releases_the_lock`, `unwind_releases_the_lock`, `failed_release_on_drop_does_not_panic` (Rust) and `explicit_release_releases_exactly_once`, `all_false_options_are_invalid_argument` (TypeScript) failed. The Rust `all_false_options_are_invalid_argument` was not seen failing. No bridge has run on a device, browser or desktop session: runtime behavior of the platform bridges is unverified and stays blocked on real hardware. [docs/parity.md](docs/parity.md) tracks the evidence.

## Approach

The brief's recommended path, built: keepawake-rs behavior on desktop, the native idle timer and window flag on mobile, the Screen Wake Lock API on web. Acquire a guard and release it on drop. See [docs/decisions.md](docs/decisions.md).

## Layout

- `crates/nsh-wake-lock/`: the Rust crate — API in `lib.rs`, `options.rs`, `error.rs`, `backend.rs`, `lock.rs`, `fake.rs`; platform bridges under `src/platform/`. The behavior tests land in `tests/` with this change set.
- `packages/nsh-wake-lock/`: the TypeScript package — `src/` (errors, options, backend, lock, tags, web, memory). The tests land in `tests/` with this change set.
- `docs/`: `brief.md` (research), `goals.md` (acceptance criteria, error codes, edge cases), `decisions.md`, `parity.md` (status).
- `site/`: static status page, published by `.github/workflows/pages.yml`.
- `.github/workflows/ci.yml`: compiles and tests everything.

## Checks

Nothing is built on a developer machine. CI runs: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets`; a per-target-triple `cargo clippy` (lib only) for each platform bridge; `npm run typecheck`; `npm test`. Bridge runtime behavior is not exercised on runners, and no claim may say it was.

## Licence

MIT. See [LICENSE](LICENSE).

Repository: https://github.com/SuperNSH/wake-lock
