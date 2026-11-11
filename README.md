# nsh-wake-lock

Keep the screen or the machine awake while the app needs it, and release it reliably.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/wake-lock/.

## Status

**Implemented, unverified.** This build delivers the Rust crate in `crates/nsh-wake-lock` (options, error model, lock and guard with release on drop, backend trait, fake backend, platform bridges for macOS, iOS, Android, Windows, Linux and web) and the TypeScript package in `packages/nsh-wake-lock` (the same error model and lock, expo-style tags, web and memory backends). The named tests for every edge case in [docs/goals.md](docs/goals.md) land in this same change set. No CI run has passed for it yet, so every capability claim above is unverified until a run on `master` passes. [docs/parity.md](docs/parity.md) tracks the evidence.

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
