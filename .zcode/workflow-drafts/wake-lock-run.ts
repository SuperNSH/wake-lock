// Fresh dynamic-workflow script: build the nsh-wake-lock Rust crate and TypeScript package
// to parity with wakelock_plus (Flutter), expo-keep-awake (React Native) and keepawake-rs,
// laid out like SuperNSH/biometrics. Every area runs its own chain (implement, gate, audit
// up to 3 rounds until zero issues) inside one fan-out; the whole-tree gate runs once at the
// join. All subagents pinned to the main-tier model per the brief. No commits, no pushes.

const MODELS = {
  pro: model("account:zai-individual-coding-plan/GLM-5.3$high"),
};

const KIT = "/Users/hmziq/ZCodeProject/ai/harness/zcode/agents";

async function specialist(name: string, fallback: string): Promise<{ persona: string; source: string }> {
  const adapter =
    "You are the " + name + " specialist for one area of a parallel workflow. " +
    "Your full specialist contract follows verbatim — apply it to this area's work; " +
    "where it conflicts with your ask, the ask wins." + "\n\n";
  for (const p of [".zcode/agents/" + name + ".md", KIT + "/" + name + ".md"]) {
    try {
      const raw = await files.read(p);
      return { persona: adapter + raw.replace(/^---\n[\s\S]*?\n---\n/, "").trim(), source: p };
    } catch {
      // try the next source in the chain
    }
  }
  return { persona: fallback, source: "fallback" };
}

// ---------------------------------------------------------------- shared spec

const SPEC = [
  "PROJECT. Repo SuperNSH/wake-lock at the checkout root. Build the nsh-wake-lock Rust crate and",
  "TypeScript package to behavior parity with three references already cloned under .refs/ (read",
  "them, never edit them): Flutter wakelock_plus (.refs/wakelock_plus/wakelock_plus/lib/wakelock_plus.dart",
  "and .refs/wakelock_plus/wakelock_plus_platform_interface/lib/), React Native expo-keep-awake",
  "(.refs/expo/packages/expo-keep-awake/src/), and Rust keepawake-rs (.refs/keepawake-rs/src/lib.rs and",
  ".refs/keepawake-rs/src/sys/). The repo layout mirrors SuperNSH/biometrics (.refs/biometrics/): a",
  "cargo workspace with the crate in crates/nsh-wake-lock, the TypeScript package in",
  "packages/nsh-wake-lock, docs in docs/, CI in .github/workflows/ci.yml. NSH itself (.refs/nsh) has no",
  "wake-lock capability today, so this crate is a new capability in the org's style, not a wrapper of",
  "an existing NSH API.",
  "",
  "ERROR MODEL (predesigned; identical in Rust and TypeScript). ErrorKind has exactly these kinds,",
  "stable codes and one-sentence messages:",
  "- NotSupported / ERR_WAKE_LOCK_NOT_SUPPORTED / Wake locks are not supported on this platform or browser.",
  "- InvalidArgument / ERR_WAKE_LOCK_INVALID_ARGUMENT / The options do not request any lock: set display, idle or sleep.",
  "- NoActivity / ERR_WAKE_LOCK_NO_ACTIVITY / There is no window or activity to hold the lock on right now.",
  "- Backgrounded / ERR_WAKE_LOCK_BACKGROUNDED / The app is backgrounded, so the idle-timer change does not apply now.",
  "- Timeout / ERR_WAKE_LOCK_TIMEOUT / The platform took too long to answer. Try again.",
  "- NotAcquired / ERR_WAKE_LOCK_NOT_ACQUIRED / That lock is not held: it was released already or never acquired.",
  "- Backend / ERR_WAKE_LOCK_BACKEND / The platform's wake lock service failed.",
  "WakeLockError carries kind plus optional status: Option<i64> holding the platform's own code where",
  "it gives one; code() returns the stable string. Rust models it like biometrics' error.rs with",
  "thiserror; TypeScript models it like biometrics' errors.ts (class WakeLockError extends Error with a",
  "readonly code field, plus wakeLockError(code) and asWakeLockError(unknown) that maps anything",
  "unrecognized to ERR_WAKE_LOCK_BACKEND).",
  "",
  "RUST API (predesigned).",
  "- options.rs: pub struct WakeLockOptions, non_exhaustive, Default with all flags false. Fields:",
  "  display: bool, idle: bool, sleep: bool, reason: Option<String>, app_name: Option<String>,",
  "  app_reverse_domain: Option<String>. Semantics from keepawake-rs: display prevents display sleep,",
  "  idle prevents idle system sleep, sleep prevents system sleep. Bridges apply defaults where the",
  "  platform wants them: reason defaults to \"User requested\", app_name to \"nsh-wake-lock\",",
  "  app_reverse_domain to \"io.github.supernsh.wake-lock\".",
  "- backend.rs: pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>; pub struct",
  "  LockId(u64) with a private field; trait Backend: Send + Sync with four methods:",
  "  is_supported(&self) -> BackendFuture<'_, Result<bool, WakeLockError>>;",
  "  acquire(&self, options: &WakeLockOptions) -> BackendFuture<'_, Result<LockId, WakeLockError>>;",
  "  release(&self, id: LockId) -> Result<(), WakeLockError> (synchronous: every platform's release",
  "  call is sync-friendly, and Drop needs it);",
  "  is_active(&self) -> BackendFuture<'_, Result<bool, WakeLockError>>.",
  "- lock.rs: WakeLock with new() (platform default backend behind a OnceLock, like biometrics'",
  "  LocalAuth::new), with_backend(impl Backend + 'static), is_supported(), acquire(&self, options) ->",
  "  Result<WakeLockGuard, WakeLockError> which returns InvalidArgument when display, idle and sleep",
  "  are all false, and is_active() (wakelock_plus enabled parity). WakeLockGuard holds the LockId,",
  "  release(mut self) -> Result<(), WakeLockError> calls backend.release once and disarms, and Drop",
  "  releases while armed (result ignored, with a 1-2 line comment saying release on drop must never",
  "  panic). Multiple guards may be held at once; there is no single-flight gate.",
  "- fake.rs: FakeBackend (Clone, Arc<Mutex<state>> like biometrics' fake.rs) with new() (supported),",
  "  set_supported(Result<bool, WakeLockError>), a per-call scripted acquire answer (default Ok with a",
  "  fresh id), fail_next_release(Option<WakeLockError>), counters acquires() and releases(),",
  "  active ids, last_options() -> Option<WakeLockOptions>, and release of an id not in its active set",
  "  returns NotAcquired. is_active is true while any lock it issued is unreleased.",
  "- lib.rs: crate docs (what it does, release on drop, resolve exactly once), module wiring,",
  "  re-exports: WakeLock, WakeLockGuard, WakeLockOptions, Backend, BackendFuture, LockId, FakeBackend,",
  "  WakeLockError, ErrorKind. Inline unit tests only for the code table and option defaults;",
  "  behavior coverage lives in tests/ owned by another area.",
  "- platform/mod.rs: pub(crate) fn default_backend() -> std::sync::Arc<dyn Backend> dispatching by cfg",
  "  in this order: target_vendor apple -> apple::backend(); target_os android -> android::backend();",
  "  target_os windows -> windows::backend(); target_os linux -> linux::backend(); target_arch wasm32",
  "  -> web::backend(); otherwise unsupported::backend(). Each bridge file exports exactly",
  "  pub(crate) fn backend() -> std::sync::Arc<dyn Backend> and nothing else public. Until a bridge's",
  "  own area lands it, its file holds an honest stub (documented as a stub): is_supported Ok(false),",
  "  acquire Err(NotSupported), release Ok(()), is_active Ok(false). Stubs never fake success.",
  "",
  "PLATFORM BRIDGES (each in its own file, mirroring the reference platform code):",
  "- platform/apple.rs, cfg split: on macOS mirror .refs/keepawake-rs/src/sys/macos.rs and its iokit",
  "  module — IOPMAssertionCreateWithName per requested flag (display uses",
  "  kIOPMAssertionTypePreventUserIdleDisplaySleep, idle uses PreventUserIdleSystemSleep, sleep uses",
  "  PreventSystemSleep), IOPMAssertionRelease on release and drop, extern declarations handwritten",
  "  with core-foundation CFString, nonzero IOReturn maps to Backend with status set to the result code.",
  "  On iOS mirror wakelock_plus's idle-timer behavior: UIApplication idleTimerDisabled true on",
  "  acquire(display), false on release; when the app is backgrounded, acquire returns Backgrounded",
  "  (this is the brief's iOS edge case); no application context returns NotSupported; idle and sleep",
  "  flags are accepted and ignored there. Use objc2-family crates for the iOS side; if the exact",
  "  framework crate is unavailable on crates.io, fall back to objc2 core with fully documented",
  "  SAFETY comments.",
  "- platform/android.rs: mirror wakelock_plus Android: through ndk-context get the VM and activity,",
  "  set or clear WindowManager FLAG_KEEP_SCREEN_ON on the current window via JNI (jni crate). No",
  "  activity or context maps to NoActivity; any JNI failure maps to Backend with the JNI status. Only",
  "  display applies; idle and sleep are accepted and ignored. Flags apply to the window, not the app.",
  "- platform/windows.rs: mirror .refs/keepawake-rs/src/sys/windows.rs: PowerCreateRequest plus",
  "  PowerSetRequest with the display/system/away-mode request contexts matching the flags,",
  "  PowerClearRequest and the handle close on release; failures map to Backend with the last error",
  "  status.",
  "- platform/linux.rs: mirror .refs/keepawake-rs/src/sys/linux.rs with the zbus crate: acquire an",
  "  inhibit over org.freedesktop.ScreenSaver (cookie) and GNOME org.gnome.SessionManager as the",
  "  reference does, keep the cookie as the LockId, release via the matching UnInhibit. Every D-Bus",
  "  call is bounded by a tokio::time::timeout of 5 seconds that maps to Timeout so acquire never",
  "  hangs; release is sync (zbus blocking facade or an equivalent that cannot deadlock inside an",
  "  async context); failures map to Backend with status where the service gives one.",
  "- platform/web.rs: the Screen Wake Lock API through wasm-bindgen and js-sys: navigator.wakeLock",
  "  missing means NotSupported; request(\"screen\") gives a sentinel kept as the lock; display only,",
  "  idle and sleep accepted and ignored; release calls sentinel.release() fire-and-forget with the",
  "  promise rejection swallowed in Drop and surfaced as Backend on explicit release; is_active",
  "  reports an unreleased sentinel.",
  "",
  "TYPESCRIPT PACKAGE (predesigned; packages/nsh-wake-lock):",
  "- package.json: name nsh-wake-lock, version 0.1.0, type module, main and types ./src/index.ts,",
  "  exports map with \".\" -> ./src/index.ts and \"./memory\" -> ./src/memory.ts, scripts: typecheck =",
  "  tsc --noEmit, test = vitest run --passWithNoTests, devDependencies typescript and vitest pinned",
  "  to current stable (verify with npm view before pinning). Run npm install so package-lock.json",
  "  exists; commit nothing but leave the lockfile in the tree for CI.",
  "- tsconfig.json: strict true, noEmit, target ES2022, lib ES2022 + DOM, module ESNext,",
  "  moduleResolution Bundler, include src and tests. Self-contained (no .nsh extends).",
  "- src/errors.ts: WakeLockErrorCode union of the 7 codes, MESSAGES with the same wording as Rust,",
  "  class WakeLockError extends Error with readonly code, wakeLockError(code), asWakeLockError(unknown).",
  "- src/options.ts: interface WakeLockOptions with optional display, idle, sleep, reason, appName,",
  "  appReverseDomain (camelCase mirror).",
  "- src/backend.ts: interface LockHandle { readonly id: number }; interface WakeLockBackend with",
  "  isSupported(): Promise<boolean>, acquire(options): Promise<LockHandle>, release(handle): void,",
  "  isActive(): Promise<boolean>.",
  "- src/lock.ts: class WakeLock (constructor takes a WakeLockBackend; isSupported, acquire returning",
  "  WakeLockGuard, isActive) and class WakeLockGuard with release(): void that releases exactly once",
  "  and is a documented no-op afterwards; acquire with all flags unset throws WakeLockError",
  "  ERR_WAKE_LOCK_INVALID_ARGUMENT.",
  "- src/tags.ts: expo parity. const ExpoKeepAwakeTag = \"ExpoKeepAwakeDefaultTag\"; activateKeepAwakeAsync(tag?)",
  "  acquires a display lock per tag (re-activating a tag releases the old lock first so one lock per",
  "  tag); deactivateKeepAwake(tag?) releases, and an unknown tag rejects with ERR_WAKE_LOCK_NOT_ACQUIRED",
  "  (a deliberate difference from expo's console.warn — parity.md records it); isAvailableAsync()",
  "  answers backend.isSupported(); enable(), disable() and toggle(enable) are wakelock_plus parity over",
  "  one module-level lock and are idempotent. setTagBackend(backend) lets tests inject a backend; the",
  "  default is the web backend. expo's useKeepAwake hook and addListener are not provided in v0 (no",
  "  React peer dependency); parity.md documents this.",
  "- src/web.ts: webBackend(): WakeLockBackend over navigator.wakeLock with feature detection,",
  "  mirroring platform/web.rs behavior.",
  "- src/memory.ts: memoryBackend(config?) for tests: optional supported flag, scripted acquire",
  "  answers and release failures, counters acquires(), releases(), lastOptions(), release of an",
  "  unknown handle throws ERR_WAKE_LOCK_NOT_ACQUIRED.",
  "- src/index.ts: re-export the public surface.",
  "",
  "EDGE CASES (predesigned; each maps to one named test in BOTH languages):",
  "1. drop_releases_the_lock — a dropped guard releases exactly once (the brief's crash-scope case).",
  "2. unwind_releases_the_lock — a panic unwinding through a scope still releases (catch_unwind).",
  "3. explicit_release_releases_exactly_once — release then drop touches the backend once.",
  "4. all_false_options_are_invalid_argument — the battery case: no lock kind requested is an error.",
  "5. every_kind_carries_its_stable_code — the code table.",
  "6. unsupported_platform_reports_not_supported — the web-browser-without-wake-lock case.",
  "7. backgrounded_reports_backgrounded — the brief's iOS backgrounded idle-timer case.",
  "8. missing_activity_reports_no_activity — the brief's Android window-not-app case.",
  "9. unknown_handle_release_reports_not_acquired — stale or double release.",
  "10. options_reach_the_backend — flags and reason/app_name/app_reverse_domain pass through.",
  "11. is_active_reports_whether_a_lock_is_held — wakelock_plus enabled parity.",
  "12. scripted_backend_error_surfaces_with_code — a failing backend surfaces Backend plus its status.",
  "TypeScript adds: tests/tags.test.ts with activate_then_deactivate_per_tag,",
  "reactivating_a_tag_replaces_one_lock, deactivate_of_unknown_tag_reports_not_acquired,",
  "enable_disable_and_toggle_are_idempotent; tests/web.test.ts with",
  "is_available_async_reports_false_without_navigator_wake_lock.",
  "",
  "GATES (the brief's gates; the script runs them, scoped per area): cargo fmt --all -- --check;",
  "cargo clippy --all-targets -- -D warnings; cargo test --all-targets; TypeScript typecheck and",
  "tests via npm. Cross-platform bridges are compile-verified per target triple with cargo clippy",
  "(lib, no --all-targets) into separate target dirs; their runtime behavior cannot be shown on this",
  "machine and no report may claim otherwise.",
].join("\n");

const RULES = [
  "REPO RULES (AGENTS.md, hard): comments are 1-2 lines and state a non-obvious constraint; dead code",
  "is deleted, no speculative abstractions; every behavior change has a test; an operation resolves",
  "exactly once (success, a coded failure, or a cancel — it never hangs); status claims in README and",
  "docs must match what CI runs, and with no CI run yet every claim is worded as unverified; docs read",
  "like a competent human wrote them — direct, concrete, no filler.",
  "PROCESS RULES: work directly in the current checkout at /Users/hmziq/fo/nsh-wake-lock — no branches,",
  "no worktrees, no commits, no pushes, no git state changes. Touch only the files this area owns.",
  "Read .refs/ freely but never edit it. No emulator, simulator or device runs. Run only scoped checks",
  "for your own files while working; the script runs the shared gates after you, so never run the",
  "whole-tree suite yourself. If a check is impossible to pass or your instructions contradict each",
  "other, say so plainly in blockers rather than working around it.",
].join("\n");

// ---------------------------------------------------------------- types

interface ImplDone {
  /** False when the area could not finish its plan. */
  done: boolean;
  /** What was implemented, one or two sentences, honest about gaps. */
  summary: string;
  /** Files changed, workspace-relative. */
  changed: string[];
  /** Anything that could not be done and why. Empty when none. */
  blockers: string[];
  /** One sentence copied verbatim from your instructions that most shaped the work. */
  contractLine: string;
}

interface Finding {
  /** Workspace-relative path, with a line when it applies: "src/a.rs:42". */
  path: string;
  /** One sentence: what is wrong. */
  problem: string;
  /** How much it matters. Reserve high for a crash, a hang, or a wrong result. */
  severity: "low" | "medium" | "high";
}

interface Audit {
  /** Actionable findings that would block done. Empty when none. No PASS notes. */
  findings: Finding[];
  /** One sentence copied verbatim from your instructions that most shaped the audit. */
  contractLine: string;
}

interface GateOutcome {
  /** The command as the user would read it. */
  label: string;
  exitCode: number;
  stderr: string;
}

interface AreaResult {
  name: string;
  status: "done" | "not converged" | "failed";
  /** What the area implemented; what failed or never converged. Honest. */
  summary: string;
  rounds: number;
  blockers: string[];
}

// ---------------------------------------------------------------- gate helpers

async function cargoFmt(): Promise<GateOutcome> {
  const run = await world.run("cargo", ["fmt", "--all", "--", "--check"], { timeoutMs: 120_000 });
  return { label: "cargo fmt --all -- --check", exitCode: run.exitCode, stderr: run.stderr };
}

async function cargoHostAll(): Promise<GateOutcome[]> {
  const out: GateOutcome[] = [await cargoFmt()];
  const clippy = await world.run("cargo", ["clippy", "--all-targets", "--", "-D", "warnings"], { timeoutMs: 600_000 });
  out.push({ label: "cargo clippy --all-targets -- -D warnings", exitCode: clippy.exitCode, stderr: clippy.stderr });
  const test = await world.run("cargo", ["test", "--all-targets"], { timeoutMs: 600_000 });
  out.push({ label: "cargo test --all-targets", exitCode: test.exitCode, stderr: test.stderr });
  return out;
}

async function crossClippy(triple: string, dir: string): Promise<GateOutcome> {
  const run = await world.run(
    "cargo",
    ["clippy", "--target", triple, "--target-dir", dir, "--", "-D", "warnings"],
    { timeoutMs: 900_000 },
  );
  return { label: "cargo clippy --target " + triple + " -- -D warnings", exitCode: run.exitCode, stderr: run.stderr };
}

async function tsGates(): Promise<GateOutcome[]> {
  const typecheck = await world.run("npm", ["--prefix", "packages/nsh-wake-lock", "run", "typecheck"], { timeoutMs: 300_000 });
  const out: GateOutcome[] = [
    { label: "npm --prefix packages/nsh-wake-lock run typecheck", exitCode: typecheck.exitCode, stderr: typecheck.stderr },
  ];
  const test = await world.run("npm", ["--prefix", "packages/nsh-wake-lock", "test"], { timeoutMs: 600_000 });
  out.push({ label: "npm --prefix packages/nsh-wake-lock test", exitCode: test.exitCode, stderr: test.stderr });
  return out;
}

async function ciYamlGate(): Promise<GateOutcome> {
  const run = await world.run(
    "ruby",
    ["-ryaml", "-e", "YAML.load_file(ARGV[0]); puts 'ci.yml parses'", ".github/workflows/ci.yml"],
    { timeoutMs: 60_000 },
  );
  return { label: "ruby yaml parse of .github/workflows/ci.yml", exitCode: run.exitCode, stderr: run.stderr };
}

// ---------------------------------------------------------------- ask builders

function auditAsk(areaName: string, owns: string, round: number, gateFails: GateOutcome[], prior: Finding[]): string {
  const gateText =
    gateFails.length === 0
      ? "The script's gates are currently green."
      : "The script's gates currently fail:\n" +
        gateFails.map((g) => g.label + " exited " + g.exitCode + "\n" + g.stderr.slice(-2500)).join("\n---\n");
  const priorText =
    prior.length === 0
      ? "No prior findings."
      : "Findings from the earlier round (check they are now fixed):\n" + JSON.stringify(prior);
  return (
    "Audit round " + round + " of " + areaName + ". You audit only: do not edit any file, and do not run " +
    "cargo, npm or any gate yourself — the script runs the gates and reports their output here.\n" +
    "Scope: " + owns + ". Read every file in scope and judge it against the spec and rules below. " +
    "Return ONLY actionable findings that would block done — spec violations, possible panics, " +
    "possible hangs, operations that do not resolve exactly once, unsound unsafe, missing behavior " +
    "the spec names, AGENTS.md rule breaks, or doc claims that outrun the evidence. An interface " +
    "whose array is counted must not carry PASS notes: empty findings means you found nothing.\n\n" +
    gateText + "\n\n" + priorText + "\n\nSPEC AND RULES:\n" + SPEC + "\n\n" + RULES + "\n\n" +
    "In contractLine, copy one sentence verbatim from this ask or the specialist contract that most shaped the audit."
  );
}

function fixAsk(owns: string, gateFails: GateOutcome[], findings: Finding[]): string {
  return (
    "Fix what verification surfaced. You own exactly these files: " + owns + " — edit nothing else. " +
    "If a failure is in a file you do not own, do not fix it there: record it in blockers.\n" +
    (gateFails.length === 0
      ? "The gates are green; the findings below are the work."
      : "Gate failures:\n" + gateFails.map((g) => g.label + " exited " + g.exitCode + "\n" + g.stderr.slice(-3500)).join("\n---\n")) +
    "\nFindings:\n" + JSON.stringify(findings) + "\n\n" + RULES + "\n" +
    "Re-run the scoped checks for your own files after fixing."
  );
}

async function verifyContract(persona: string, line: string, who: ReturnType<typeof agent>): Promise<boolean> {
  const given = typeof line === "string" ? line.trim() : "";
  if (given.length > 0 && persona.includes(given)) return true;
  const retry = await who.ask<ImplDone>(
    "Your contractLine did not quote your instructions verbatim. Reply with one full sentence copied " +
      "exactly as written from your instructions, no paraphrase.",
  );
  return persona.includes(String(retry.contractLine ?? "").trim());
}

// ---------------------------------------------------------------- area chains

interface ChainDeps {
  name: string;
  owns: string;
  ask: string;
  /** Specialist file for the implementer persona; null uses the inline fallback. */
  implSpecialist: string | null;
  implFallback: string;
  implAgentName: string;
  /** Specialist file for the auditor persona; null uses the inline text. */
  auditSpecialist: string | null;
  auditInline: string;
  auditAgentName: string;
  gates: (() => Promise<GateOutcome[]>) | null;
}

async function runChain(d: ChainDeps, ...deps: Promise<AreaResult>[]): Promise<AreaResult> {
  try {
    for (const dep of deps) {
      const depResult = await dep;
      if (depResult.status !== "done") {
        const summary = "skipped: dependency " + depResult.name + " ended " + depResult.status;
        report({ name: d.name, phase: "failed", summary }, "areas");
        return { name: d.name, status: "failed", summary, rounds: 0, blockers: [depResult.summary] };
      }
    }
    report({ name: d.name, phase: "implementing", summary: d.owns }, "areas");
    const implPersona = d.implSpecialist !== null
      ? (await specialist(d.implSpecialist, d.implFallback)).persona
      : d.implFallback;
    const auditPersona = d.auditSpecialist !== null
      ? (await specialist(d.auditSpecialist, d.auditInline)).persona
      : d.auditInline;
    const impl = agent(d.implAgentName, { system: implPersona, model: MODELS.pro });
    const auditor = agent(d.auditAgentName, { system: auditPersona, model: MODELS.pro });
    const first = await impl.ask<ImplDone>(d.ask + "\n\nTHE PREDESIGNED SPEC EVERY AREA BUILDS AGAINST:\n" + SPEC);
    const lineOk = await verifyContract(implPersona, first.contractLine, impl);
    let gateFails: GateOutcome[] = [];
    let findings: Finding[] = [];
    let converged = false;
    let rounds = 0;
    let summary = first.summary;
    const blockers = first.blockers.slice();
    for (let round = 1; round <= 3; round++) {
      rounds = round;
      report({ name: d.name, phase: "auditing", summary: "gates and audit round " + round }, "areas");
      if (d.gates !== null) {
        gateFails = (await d.gates()).filter((g) => g.exitCode !== 0);
      }
      const audit = await auditor.ask<Audit>(auditAsk(d.name, d.owns, round, gateFails, findings));
      if (audit.findings.length === 0 && gateFails.length === 0) {
        converged = true;
        break;
      }
      findings = audit.findings;
      if (round < 3) {
        report({ name: d.name, phase: "fixing", summary: findings.length + " findings, " + gateFails.length + " gate failures" }, "areas");
        const fix = await impl.ask<ImplDone>(fixAsk(d.owns, gateFails, findings));
        summary = fix.summary;
        for (const b of fix.blockers) if (!blockers.includes(b)) blockers.push(b);
      }
    }
    if (!converged) {
      blockers.push(
        "after round 3: " + findings.length + " audit findings and " + gateFails.length +
          " gate failures remained; the last fix round did not run, so they stand unconfirmed",
      );
    }
    const personaNote = lineOk ? "" : " (persona delivery unverified)";
    const status: AreaResult["status"] = converged ? "done" : "not converged";
    report({ name: d.name, phase: status, summary }, "areas");
    return { name: d.name, status, summary: summary + personaNote, rounds, blockers };
  } catch (error) {
    const summary = "failed: " + String(error);
    report({ name: d.name, phase: "failed", summary }, "areas");
    return { name: d.name, status: "failed", summary, rounds: 0, blockers: [String(error)] };
  }
}

const READONLY_AUDITOR = "You audit this area read-only: never edit a file, never run cargo, npm or any gate yourself.";

function chainDocsGoals(): Promise<AreaResult> {
  const owns = "docs/goals.md, docs/decisions.md, README.md, site/index.html";
  const ask = [
    "Write the project's goal and decision documents in /Users/hmziq/fo/nsh-wake-lock. You own exactly: " +
      owns + ". Read AGENTS.md, docs/brief.md, the current docs stubs, and the references in .refs/ first.",
    "",
    "docs/goals.md (new): goals, acceptance criteria (the gates in the spec, resolve-exactly-once,",
    "never panic, never hang, coded errors, both languages tested), the error-kind table with stable",
    "codes exactly as the spec lists them, and the edge-case list where every case maps to its named",
    "test exactly as the spec names them, covering everything the Flutter and React Native references",
    "handle (read their sources; if they handle an edge case the predesigned list misses, add it with",
    "a new named test and a note that the test areas were told via this document).",
    "",
    "docs/decisions.md (rewrite the Open and Decided sections): record as decided — (1) library",
    "approach: a thin own crate mirroring keepawake-rs behavior on desktop plus native idle-timer and",
    "window-flag behavior on mobile plus the web Screen Wake Lock API, per docs/brief.md's recommended",
    "path, instead of depending on a third-party wake crate, so error codes and behavior stay uniform;",
    "(2) NSH version: the crate does not depend on nsh-core yet — NSH native-module registration is",
    "deferred, with the reason (local gates stay light; wiring into NSH apps is a follow-up); (3)",
    "platforms in scope: macOS, iOS, Android, Windows, Linux, web; (4) expo's useKeepAwake hook and",
    "addListener are not provided in v0 and why; (5) references: all five URLs cloned into .refs/ on",
    "2026-11-09, canonical repos recorded (fluttercommunity/wakelock_plus, expo/expo",
    "packages/expo-keep-awake, rustdesk-org/keepawake-rs, longbridge/nsh, SuperNSH/biometrics). Delete",
    "the open items this run decided; leave nothing open that the spec settles.",
    "",
    "README.md and site/index.html: their Status sections currently say scaffold. Update them to say",
    "what this build delivers (crate plus package plus tests) while wording every claim as unverified",
    "until a CI run passes — AGENTS.md requires status claims to match CI, and nothing is pushed in",
    "this session. Update README's Layout list (crates/, packages/) and Checks section to name the",
    "gates the updated CI runs. Keep the docs direct and filler-free.",
    "",
    "The script audits you up to three rounds until zero findings. There is no deterministic gate for",
    "docs. " + RULES,
  ].join("\n");
  return runChain({
    name: "goals docs",
    owns,
    ask,
    implSpecialist: "z-nsh-engineer",
    implFallback: "You are a careful technical writer and NSH ecosystem engineer.",
    implAgentName: "Goals docs",
    auditSpecialist: "z-nsh-engineer",
    auditInline:
      READONLY_AUDITOR + " You judge planning and status documents against the references and the spec.",
    auditAgentName: "Goals docs audit",
    gates: null,
  });
}

function chainDocsParity(): Promise<AreaResult> {
  const owns = "docs/parity.md";
  const ask = [
    "Rewrite docs/parity.md in /Users/hmziq/fo/nsh-wake-lock. You own exactly: docs/parity.md. Model the",
    "document on .refs/biometrics/docs/parity.md (read it first): one table per reference API surface,",
    "status words Done / Partial / Accepted / Platform only / Not provided, and a Covered by column",
    "naming the test that covers the row.",
    "",
    "Cover every public API of: (1) Flutter wakelock_plus — read",
    ".refs/wakelock_plus/wakelock_plus/lib/wakelock_plus.dart and the platform interface: enable,",
    "disable, toggle, enabled; (2) expo-keep-awake — read .refs/expo/packages/expo-keep-awake/src/:",
    "ExpoKeepAwakeTag, isAvailableAsync, useKeepAwake, activateKeepAwake, activateKeepAwakeAsync,",
    "deactivateKeepAwake, addListener, KeepAwakeOptions/KeepAwakeEvent/KeepAwakeListener types;",
    "(3) keepawake-rs — read .refs/keepawake-rs/src/lib.rs: Builder methods display, idle, sleep,",
    "reason, app_name, app_reverse_domain, create, and AwakeHandle.set_display.",
    "",
    "For each row name what ours does: the Rust item and the TypeScript item (per the spec below),",
    "with the honest status — accepted-and-ignored flags are Accepted, device-only behavior is",
    "Platform only, and useKeepAwake/addListener are Not provided in v0 with the workaround named.",
    "Add an error-mapping table: expo UnavailabilityError, Android's dead-activity rejection,",
    "keepawake-rs IOReturn and D-Bus failures, each mapped to its stable code. State the evidence",
    "rule from AGENTS.md at the top: rows are claims until a CI run passes, so mark the document",
    "verified-pending-CI. Name the exact test from the spec's edge-case list in Covered by.",
    "",
    "The script audits you up to three rounds until zero findings. " + RULES,
  ].join("\n");
  return runChain({
    name: "parity docs",
    owns,
    ask,
    implSpecialist: "z-nsh-engineer",
    implFallback: "You are a careful technical writer and cross-framework API mapper.",
    implAgentName: "Parity docs",
    auditSpecialist: "z-nsh-engineer",
    auditInline:
      READONLY_AUDITOR + " You judge parity documentation against the reference sources and the spec, checking every name it claims exists actually exists.",
    auditAgentName: "Parity docs audit",
    gates: null,
  });
}

function chainCore(): Promise<AreaResult> {
  const owns =
    "Cargo.toml (workspace root), crates/nsh-wake-lock/Cargo.toml, crates/nsh-wake-lock/src/lib.rs, " +
    "crates/nsh-wake-lock/src/error.rs, crates/nsh-wake-lock/src/options.rs, crates/nsh-wake-lock/src/backend.rs, " +
    "crates/nsh-wake-lock/src/lock.rs, crates/nsh-wake-lock/src/fake.rs, crates/nsh-wake-lock/src/platform/mod.rs, " +
    "crates/nsh-wake-lock/src/platform/unsupported.rs, and stub files crates/nsh-wake-lock/src/platform/apple.rs, " +
    "android.rs, windows.rs, linux.rs, web.rs";
  const ask = [
    "Build the Rust core in /Users/hmziq/fo/nsh-wake-lock following the spec below exactly. You own exactly: " +
      owns + ". The platform bridge files are created by you ONLY as honest stubs with the exact stub",
    "contract in the spec (each exporting pub(crate) fn backend()) — their real implementations belong",
    "to other areas that start after you land; do not implement them.",
    "",
    "Convert the root Cargo.toml to the workspace shape in the spec (members, resolver 3,",
    "workspace.package) and create crates/nsh-wake-lock/Cargo.toml with thiserror plus the per-target",
    "dependency sections the spec names (pin versions you verify with cargo search or cargo info;",
    "dev-deps tokio for #[tokio::test]). Keep edition 2024. The stub platform files must compile on",
    "this macOS host, so apple.rs's stub compiles for every apple target with no platform APIs yet.",
    "",
    "Implement error.rs, options.rs, backend.rs, lock.rs, fake.rs, platform/mod.rs,",
    "platform/unsupported.rs and lib.rs per the spec: same names, same shapes. Follow biometrics'",
    "conventions (.refs/biometrics/crates/nsh-biometrics/src/) for doc-comment style, error.rs",
    "structure, fake.rs structure and the OnceLock platform default.",
    "",
    "Every behavior you land carries a test (inline for the code table and defaults). The script gates",
    "you with cargo fmt --all -- --check, cargo clippy --all-targets -- -D warnings and cargo test",
    "--all-targets after each round, then audits you until zero findings (up to 3 rounds). Run scoped",
    "checks yourself while working. " + RULES,
  ].join("\n");
  return runChain({
    name: "rust core",
    owns,
    ask,
    implSpecialist: "z-rust-engineer",
    implFallback: "You are a careful Rust library engineer.",
    implAgentName: "Rust core",
    auditSpecialist: "z-rust-engineer",
    auditInline:
      READONLY_AUDITOR + " You judge Rust library code: API surface, error design, ownership, drop and unwind behavior, mirror fidelity with the spec.",
    auditAgentName: "Rust core audit",
    gates: cargoHostAll,
  });
}

interface BridgeSpec {
  platform: string;
  file: string;
  gates: () => Promise<GateOutcome[]>;
  brief: string;
}

function chainBridge(bs: BridgeSpec, core: Promise<AreaResult>): Promise<AreaResult> {
  const name = bs.platform + " bridge";
  const owns = bs.file;
  const ask = [
    "Replace the stub in " + bs.file + " (in /Users/hmziq/fo/nsh-wake-lock) with the real " + bs.platform +
      " bridge. You own exactly this one file; if the crate's Cargo.toml lacks a dependency you need,",
    "record that in blockers instead of editing the manifest — the spec's dependency list should",
    "already cover you.",
    "",
    "Keep the exact public seam: pub(crate) fn backend() -> std::sync::Arc<dyn Backend>. Implement the",
    "Backend trait per the spec: never panic (no unwrap or expect on platform results; map failures to",
    "coded errors, keeping the platform status), never hang (bounded platform calls), release exactly",
    "once across explicit release and Drop.",
    "",
    bs.brief,
    "",
    "Read the reference files named in the spec and mirror their behavior; adapt only where the spec",
    "differs. Accept-and-ignore flags stay accepted. Delete the stub's honest-placeholder comment when",
    "the real implementation lands; leave no dead code.",
    "",
    "The script gates you after each round with your target's gates, then audits you until zero",
    "findings (up to 3 rounds). Your platform's runtime behavior cannot be exercised on this machine:",
    "compile-verification is the bar here, and your summary must say plainly that runtime behavior is",
    "unverified off-host. Run scoped checks yourself while working. " + RULES,
  ].join("\n");
  return runChain({
    name,
    owns,
    ask,
    implSpecialist: "z-rust-engineer",
    implFallback: "You are a careful Rust platform engineer.",
    implAgentName: bs.platform + " bridge engineer",
    auditSpecialist: "z-unsafe-ffi-auditor",
    auditInline:
      READONLY_AUDITOR +
      " You judge platform bridge soundness: extern declarations, raw handles, JNI, D-Bus, JS interop, drop paths, panic and hang freedom.",
    auditAgentName: bs.platform + " bridge audit",
    gates: bs.gates,
  }, core);
}

function chainTsPackage(): Promise<AreaResult> {
  const owns =
    "packages/nsh-wake-lock/package.json, packages/nsh-wake-lock/package-lock.json, packages/nsh-wake-lock/tsconfig.json, " +
    "and packages/nsh-wake-lock/src/ (errors.ts options.ts backend.ts lock.ts tags.ts web.ts memory.ts index.ts)";
  const ask = [
    "Build the TypeScript package in /Users/hmziq/fo/nsh-wake-lock/packages/nsh-wake-lock per the spec",
    "below exactly. You own exactly: " + owns + " — not the tests directory (another area owns",
    "packages/nsh-wake-lock/tests) and not the Rust crate. Run npm install in the package directory so",
    "the lockfile exists before the gates.",
    "",
    "Mirror the Rust API faithfully: same error codes and messages, same names adapted to camelCase,",
    "same resolve-exactly-once semantics (guard release once with a documented no-op afterwards, tag",
    "deactivate of an unknown tag rejects with ERR_WAKE_LOCK_NOT_ACQUIRED, options validation matches",
    "the Rust InvalidArgument rule). web.ts feature-detects navigator.wakeLock and works in a Node",
    "test environment by reporting availability false.",
    "",
    "The script gates you with the package typecheck and vitest after each round (no test files exist",
    "yet; the test script passes with none), then audits you until zero findings (up to 3 rounds).",
    "Run scoped checks yourself while working. " + RULES,
  ].join("\n");
  return runChain({
    name: "ts package",
    owns,
    ask,
    implSpecialist: "z-react-native-engineer",
    implFallback: "You are a careful TypeScript package engineer for React Native and Expo ecosystems.",
    implAgentName: "TypeScript package engineer",
    auditSpecialist: "z-react-native-engineer",
    auditInline:
      READONLY_AUDITOR + " You judge TypeScript library code: API surface, error classes, promise behavior, mirror fidelity with the Rust API in the spec.",
    auditAgentName: "TypeScript package audit",
    gates: tsGates,
  });
}

function chainRustTests(goals: Promise<AreaResult>, core: Promise<AreaResult>, apple: Promise<AreaResult>): Promise<AreaResult> {
  const owns = "crates/nsh-wake-lock/tests/ (new integration test files only, for example lock.rs errors.rs edge_cases.rs)";
  const ask = [
    "Write the Rust integration tests in /Users/hmziq/fo/nsh-wake-lock/crates/nsh-wake-lock/tests/ (new",
    "files only). You own exactly: " + owns + ". Do not modify src/ — if a test cannot pass because of",
    "a core bug or a missing behavior, write the test to expose it, then record the failure in blockers",
    "instead of changing src.",
    "",
    "Cover every named edge case in the spec's list (read docs/goals.md too, it landed with the same",
    "names) against FakeBackend: exactly-once release semantics, drop and unwind, option validation,",
    "the full error-code table, the wakelock_plus is_active parity, options pass-through, scripted",
    "backend failures with status. Use #[tokio::test]. Every test name matches the spec's names.",
    "",
    "The script gates you with cargo fmt --all -- --check, cargo clippy --all-targets -- -D warnings",
    "and cargo test --all-targets after each round, then audits you until zero findings (up to 3",
    "rounds). " + RULES,
  ].join("\n");
  return runChain({
    name: "rust tests",
    owns,
    ask,
    implSpecialist: "z-rust-engineer",
    implFallback: "You are a careful Rust test engineer.",
    implAgentName: "Rust test writer",
    auditSpecialist: "z-rust-engineer",
    auditInline:
      READONLY_AUDITOR + " You judge test suites: coverage against the named case list, assertion strength, exact-once semantics, no fake passes.",
    auditAgentName: "Rust tests audit",
    gates: cargoHostAll,
  }, goals, core, apple);
}

function chainTsTests(goals: Promise<AreaResult>, tsPkg: Promise<AreaResult>): Promise<AreaResult> {
  const owns = "packages/nsh-wake-lock/tests/ (new test files only: lock.test.ts, tags.test.ts, web.test.ts)";
  const ask = [
    "Write the TypeScript tests in /Users/hmziq/fo/nsh-wake-lock/packages/nsh-wake-lock/tests/ (new files",
    "only). You own exactly: " + owns + ". Do not modify src/ — if a test cannot pass because of a",
    "package bug, write the test to expose it and record the failure in blockers instead of changing",
    "src.",
    "",
    "Cover every named edge case from the spec's list plus the tag and web tests the spec names,",
    "against memoryBackend (read docs/goals.md too, it landed with the same names). Test names match",
    "the spec. The web tests assert feature-detection behavior in Node (no navigator wake lock).",
    "",
    "The script gates you with the package typecheck and vitest after each round, then audits you",
    "until zero findings (up to 3 rounds). " + RULES,
  ].join("\n");
  return runChain({
    name: "ts tests",
    owns,
    ask,
    implSpecialist: "z-react-native-engineer",
    implFallback: "You are a careful TypeScript test engineer.",
    implAgentName: "TypeScript test writer",
    auditSpecialist: "z-react-native-engineer",
    auditInline:
      READONLY_AUDITOR + " You judge test suites: coverage against the named case list, assertion strength, exact-once semantics, no fake passes.",
    auditAgentName: "TypeScript tests audit",
    gates: tsGates,
  }, goals, tsPkg);
}

const CI_PERSONA =
  "You are a careful GitHub Actions workflow engineer. You keep pinned action SHAs, minimal permissions and honest job names.";

function chainCi(): Promise<AreaResult> {
  const owns = ".github/workflows/ci.yml";
  const ask = [
    "Update /Users/hmziq/fo/nsh-wake-lock/.github/workflows/ci.yml so CI runs both language gates. You",
    "own exactly this file. Keep the existing file's conventions (pinned action SHAs with version",
    "comments, concurrency group, minimal permissions, CARGO env). Read",
    ".refs/biometrics/.github/workflows/ci.yml for the org's shape.",
    "",
    "Required: (1) the rust job keeps cargo fmt --all -- --check, cargo clippy --all-targets -- -D",
    "warnings, cargo test --all-targets; (2) a typescript job: checkout, setup-node on the LTS line,",
    "npm ci in packages/nsh-wake-lock (the lockfile lands with this build), npm run typecheck, npm test;",
    "(3) a cross-check job clippy-compiling the lib per target triple like biometrics does:",
    "aarch64-apple-ios-sim, x86_64-pc-windows-gnu, x86_64-unknown-linux-gnu and wasm32-unknown-unknown",
    "directly, and aarch64-linux-android with the NDK cc/AR/linker env lines biometrics uses; command",
    "shape cargo clippy --target <triple> -- -D warnings (no --all-targets, so dev-deps stay",
    "host-only). Jobs run on push to master, pull_request and workflow_dispatch as today.",
    "",
    "The script gates you with a YAML parse check after each round, then audits you until zero findings",
    "(up to 3 rounds). " + RULES,
  ].join("\n");
  return runChain({
    name: "ci workflow",
    owns,
    ask,
    implSpecialist: null,
    implFallback: CI_PERSONA,
    implAgentName: "CI workflow engineer",
    auditSpecialist: null,
    auditInline:
      READONLY_AUDITOR + " You judge CI workflow files against the gates they must run and the repo's pinned-SHA conventions.",
    auditAgentName: "CI workflow audit",
    gates: async () => {
      const out: GateOutcome[] = [await ciYamlGate()];
      return out;
    },
  });
}

// ---------------------------------------------------------------- board + fan-out

artifact.board("areas", {
  title: "Wake lock build areas",
  key: "name",
  status: "phase",
  columns: ["implementing", "auditing", "fixing", "done", "not converged", "failed"],
});

phase("Implement and verify each area in parallel, each with its own audit rounds");

log("12 areas: docs x2, rust core, 5 platform bridges, ts package, 2 test areas, ci");

const goals = chainDocsGoals();
const parity = chainDocsParity();
const core = chainCore();
const tsPkg = chainTsPackage();

const apple = chainBridge(
  {
    platform: "Apple",
    file: "crates/nsh-wake-lock/src/platform/apple.rs",
    gates: async () => {
      const host = await cargoHostAll();
      const iosSim = await crossClippy("aarch64-apple-ios-sim", "target/xcheck/ios-sim");
      return [...host, iosSim];
    },
    brief:
      "macOS: mirror keepawake-rs's sys/macos.rs IOPMAssertion flow (display, idle, sleep assertions with the reason default; IOPMAssertionRelease on release and drop; nonzero IOReturn becomes Backend with status). iOS: mirror wakelock_plus's idle-timer behavior through the objc2 family — idleTimerDisabled true on acquire when display is set, false on release, Backgrounded when the app is not active on acquire, NotSupported with no application context; idle and sleep accepted and ignored there.",
  },
  core,
);

const android = chainBridge(
  {
    platform: "Android",
    file: "crates/nsh-wake-lock/src/platform/android.rs",
    gates: async () => {
      const fmt = await cargoFmt();
      const clippy = await crossClippy("aarch64-linux-android", "target/xcheck/android");
      return [fmt, clippy];
    },
    brief:
      "Mirror wakelock_plus Android: through ndk-context get the VM and activity and set or clear WindowManager FLAG_KEEP_SCREEN_ON on the current window over JNI. No context or activity is NoActivity; JNI failures are Backend with the status; only display applies, idle and sleep accepted and ignored; the flag belongs to the window, not the app.",
  },
  core,
);

const windows = chainBridge(
  {
    platform: "Windows",
    file: "crates/nsh-wake-lock/src/platform/windows.rs",
    gates: async () => {
      const fmt = await cargoFmt();
      const clippy = await crossClippy("x86_64-pc-windows-gnu", "target/xcheck/windows");
      return [fmt, clippy];
    },
    brief:
      "Mirror keepawake-rs's sys/windows.rs with the windows crate's Win32_System_Power: PowerCreateRequest plus PowerSetRequest for the display/system/away-mode contexts matching the flags, PowerClearRequest and handle close on release, failures mapped to Backend with the last-error status.",
  },
  core,
);

const linux = chainBridge(
  {
    platform: "Linux",
    file: "crates/nsh-wake-lock/src/platform/linux.rs",
    gates: async () => {
      const fmt = await cargoFmt();
      const clippy = await crossClippy("x86_64-unknown-linux-gnu", "target/xcheck/linux");
      return [fmt, clippy];
    },
    brief:
      "Mirror keepawake-rs's sys/linux.rs with zbus: acquire an inhibit over org.freedesktop.ScreenSaver and GNOME org.gnome.SessionManager as the reference does, keep the cookie as the LockId, release via the matching UnInhibit. Bound every D-Bus call with a tokio::time::timeout of 5 seconds mapping to Timeout so acquire never hangs, and make release sync without deadlocking inside an async context.",
  },
  core,
);

const web = chainBridge(
  {
    platform: "Web",
    file: "crates/nsh-wake-lock/src/platform/web.rs",
    gates: async () => {
      const fmt = await cargoFmt();
      const clippy = await crossClippy("wasm32-unknown-unknown", "target/xcheck/wasm");
      return [fmt, clippy];
    },
    brief:
      "Use the Screen Wake Lock API through wasm-bindgen and js-sys: missing navigator.wakeLock is NotSupported; request of the screen type gives a sentinel kept as the lock; display only with idle and sleep accepted and ignored; release calls the sentinel's release fire-and-forget (swallowed in Drop, surfaced as Backend on explicit release); is_active reports an unreleased sentinel.",
  },
  core,
);

const rustTests = chainRustTests(goals, core, apple);
const tsTests = chainTsTests(goals, tsPkg);
const ciArea = chainCi();

const areaResults: AreaResult[] = await Promise.all([
  goals,
  parity,
  core,
  tsPkg,
  apple,
  android,
  windows,
  linux,
  web,
  rustTests,
  tsTests,
  ciArea,
]);

// ---------------------------------------------------------------- whole-tree gate

phase("Check the whole tree together once, and fix what only shows up together");

let joinClean = false;
let joinRounds = 0;
const treeFixer = agent("Tree fixer", {
  system:
    "You fix integration breakage across a Rust workspace and a TypeScript package after parallel work landed. " +
    "Edit only what the failing gate output names, keep the spec's API names, never commit or push, and never touch .refs/.",
  model: MODELS.pro,
});

for (let round = 1; round <= 2; round++) {
  joinRounds = round;
  const failures: string[] = [];
  const gates = await cargoHostAll();
  for (const g of gates) {
    if (g.exitCode !== 0) failures.push(g.label + " exited " + g.exitCode + "\n" + g.stderr.slice(-3000));
  }
  const tsOut = await tsGates();
  for (const g of tsOut) {
    if (g.exitCode !== 0) failures.push(g.label + " exited " + g.exitCode + "\n" + g.stderr.slice(-3000));
  }
  if (failures.length === 0) {
    joinClean = true;
    break;
  }
  log("whole-tree gates failed: " + failures.map((f) => f.split("\n")[0]).join("; "));
  await treeFixer.ask<ImplDone>(
    "The whole-tree gates failed after all areas landed:\n" + failures.join("\n---\n") +
      "\nFix the integration breakage in the checkout (no commits, no pushes, never touch .refs/). " + RULES,
  );
}

// ---------------------------------------------------------------- report

const doneCount = areaResults.filter((r) => r.status === "done").length;
const notConverged = areaResults.filter((r) => r.status === "not converged");
const failed = areaResults.filter((r) => r.status === "failed");

await artifact.markdown(
  "report",
  [
    "# Wake lock build report",
    "",
    "Brief: build the nsh-wake-lock Rust crate and TypeScript package to parity with wakelock_plus,",
    "expo-keep-awake and keepawake-rs, laid out like SuperNSH/biometrics. " +
      doneCount + " of " + areaResults.length + " areas converged clean" +
      (notConverged.length > 0 ? ", " + notConverged.length + " did not converge in 3 audit rounds" : "") +
      (failed.length > 0 ? ", " + failed.length + " failed" : "") +
      "; the whole-tree gate " + (joinClean ? "passed" : "did not pass after " + joinRounds + " round(s)") + ".",
    "",
    "## Areas",
    "",
    ...areaResults.map(
      (r) =>
        "- **" + r.name + "** — " + r.status + " (rounds: " + r.rounds + "): " + r.summary +
        (r.blockers.length > 0 ? " Blockers: " + r.blockers.join("; ") : ""),
    ),
    "",
    "## Gates",
    "",
    "- Per area, scoped: cargo fmt/clippy/test on host, cross-target clippy per bridge target, npm typecheck and vitest for TypeScript, YAML parse for CI.",
    "- Whole tree at the join: the brief's gates (cargo fmt --all -- --check, cargo clippy --all-targets -- -D warnings, cargo test --all-targets, TypeScript typecheck and tests), " + joinRounds + " round(s).",
    "",
    "## Not covered",
    "",
    "- Runtime behavior of the Apple, Android, Windows, Linux and web bridges: compile-verified only on this machine (macOS host); no device or browser ran.",
    "- CI is green for no run: nothing was committed or pushed, so every docs status claim stays worded unverified.",
    "- NSH native-module registration (nsh-core) is deliberately deferred, recorded in docs/decisions.md.",
  ].join("\n"),
  { title: "Wake lock build report", description: "Per-area outcomes, gates, and what never converged.", primary: true },
);

const conclusion =
  "Built the nsh-wake-lock Rust crate and TypeScript package across " + areaResults.length + " parallel areas: " +
  doneCount + " converged clean, " +
  (notConverged.length > 0 ? notConverged.map((r) => r.name).join(", ") + " did not converge within 3 audit rounds; " : "") +
  (failed.length > 0 ? failed.map((r) => r.name).join(", ") + " failed; " : "") +
  "whole-tree gates " + (joinClean ? "passed after " + joinRounds + " round(s)" : "still failing after " + joinRounds + " round(s)") +
  ". Nothing committed or pushed; bridges are compile-verified only off the macOS host.";

return {
  conclusion,
  areas: areaResults,
  joinClean,
  joinRounds,
  verified: [
    "each area ran its own scoped gates plus up to 3 audit rounds with a read-only auditor",
    "whole-tree gate at the join: cargo fmt --all -- --check, cargo clippy --all-targets -- -D warnings, cargo test --all-targets, npm typecheck and vitest in packages/nsh-wake-lock",
    "cross-target clippy per bridge: aarch64-apple-ios-sim, aarch64-linux-android, x86_64-pc-windows-gnu, x86_64-unknown-linux-gnu, wasm32-unknown-unknown",
  ],
  notCovered: [
    "runtime behavior of every platform bridge off the macOS host (no device, simulator or browser ran)",
    "a green CI run: nothing was pushed, so README and docs claims stay marked unverified",
    "expo useKeepAwake hook and addListener: not provided in v0, recorded in docs/parity.md and docs/decisions.md",
  ],
};
