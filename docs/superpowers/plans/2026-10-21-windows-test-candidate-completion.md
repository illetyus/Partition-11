# Windows test candidate completion implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking. Retain the previously chosen sequential execution method; request one final independent branch review after implementation.

**Goal:** Complete the remaining approved application work and deliver a functional unsigned x64 test ZIP; leave Windows 11 physical acceptance to the user.

**Architecture:** Sandboxed renderer -> trusted Electron main consent/controller -> one bounded Windows agent -> verified DiskSpd in a contained Job Object -> exact journal-backed cleanup -> strict XML validation in main. Only an opaque one-use selection token and sanitized measurements cross the renderer boundary.

**Tech Stack:** Existing locked Electron/React/TypeScript stack, Node build runtime 24.21.0, Rust 1.99.1/MSVC x64, pinned Microsoft DiskSpd v2.3 AMD64, CycloneDX 1.5 and GitHub Actions. General prerequisites remain under `D:\Android`.

**Spec:** Approved `docs/superpowers/specs/2026-10-20-file-backed-benchmark-run-design.md`, approved `docs/superpowers/specs/2026-10-20-p2-5-scratch-recovery-journal.md`, and proposed [contract addendum](../specs/2026-10-21-test-candidate-contract-addendum.md). The addendum's preparation/disclosure decisions are presented for review, not recorded as previously approved.

## Global constraints

- Worktree: `D:\Partition 11\.worktrees\readonly-disk-preview`; preserve all existing tracked/untracked work. Baseline HEAD: `38a0d7597602bd95b8980a04458ceea10caed67b`.
- Scope: P0/P1/P2/P3 regressions plus completion of P2.5 and candidate packaging. P4 image writes, P5 formatting, P6 repair, P7 resize, P8 mount and production P9 signing/updater work remain separate roadmap phases, not implemented by this plan.
- No local DiskSpd workload, physical inventory probe or mounted-user-volume test. Local native tests use temporary directories and harmless process fixtures. Real engine workloads run only in CI temporary storage.
- Exact engine SHA-256: `dd4e57e1e8ccaf5d6437938f8aab7f17e9a1e6d8fba8a093006b7cadf16faea2`; no runtime download or binary modification.
- One 1 GiB file, one run, existing four profiles, 5 s warmup, 15 s measurement; implement preparation and nested deadlines from the addendum. Renderer cannot choose paths/switches/bounds.
- Keep `BENCHMARK_RUNTIME_ENABLED = false` through Tasks 0–5. Task 6 is the separate reviewed activation gate. No hidden production test mode may bypass selection, consent or eligibility.
- Reuse draft PR #53 and the existing feature branch for authorized CI work. No merge, signing or public publication. This planning turn creates documents only.
- Every task uses red -> green -> focused regression -> small explicit-path commit. Keep incomplete tests visible; never delete/ignore them to produce a passing status.

## Current evidence and blockers

Live inspected 2026-10-21: main has an `execute` stub returning `worker_failure`; no agent `benchmark-run` dispatch; UI availability is a literal false type. `execution.rs` currently contains four tests referring to missing execution/fixture APIs. Earlier successful local tests predate those tests and are not current-worktree completion evidence.

GitHub runs at baseline HEAD: [CI passed](https://github.com/illetyus/Partition-11/actions/runs/39461304681), [Windows UI passed](https://github.com/illetyus/Partition-11/actions/runs/39461304668), [DiskSpd failed](https://github.com/illetyus/Partition-11/actions/runs/39461304656). Local recovery changes have not yet supplied a new compatibility result. Existing [draft PR #53](https://github.com/illetyus/Partition-11/pull/53) has stale descriptions requiring correction during implementation.

## Review focus

1. Uninitialized scratch data and inaccurate read/write disclosure: Task 2 proves full preparation; Task 3 tests consent arithmetic and text.
2. Panic/undrained process makes Drop delete too early: Task 1 retains file/journal and proves recovery only after handles stop.
3. Parent death, hotplug or cancellation after primary exit: Tasks 2/5 reject stale success and wait for the entire job, not just its first process.
4. Recovery timeout or inventory refresh overlaps another worker: Tasks 3/5 retain quarantine and serialize recovery, selection and execution.
5. Candidate built from a different commit or old agent/SBOM: Tasks 6/7 bind all checks/resources/artifacts to the final source commit.

## Task 0 — Establish a truthful, buildable checkpoint

**Files:** `AGENTS.md`, `docs/PLAN.md`, `docs/SESSION_HANDOFF.md`, existing `.superpowers/sdd/2026-10-20-p2-5-production-benchmark-integration/progress.md`; read both approved specs/plans and the addendum.

**Interface:** Produce the current task ledger with source HEAD, dirty-file inventory, existing PR, exact toolchain and unresolved tests; no runtime interface change.

- [ ] Record `git status --short`, `git diff --stat`, `git branch --show-current`, and the baseline remote checks. Resolve contradictory historical checkpoint headings with a new authoritative top section; keep history labelled.
- [ ] Run the current native compile probe: `cargo test --locked -p partition11-benchmark-worker --lib execution::tests --no-run`. Record the actual missing-symbol diagnostics as the starting RED; implement them in Task 1. Do not describe the older 42-worker-test result as current green.
- [ ] Check pinned tools with `Get-Command`; load existing Rust 1.99.1/MSVC environment. Repair only task-local prerequisites under `D:\Android` if missing; do not replace locked dependencies or weaken hooks/policies.

**Exit:** All dirty work attributed, errors captured, native toolchain usable, current authority/CI state unambiguous. Commit checkpoint documentation together with the first buildable Task 1 slice.

## Task 1 — Complete lease execution, recovery and engine compatibility

**Files:** `crates/benchmark-worker/src/{execution.rs,scratch.rs,job_process.rs,lib.rs}`, `crates/benchmark-worker/src/scratch/journal.rs`, `.github/workflows/windows-diskspd-xml.yml`.

**Interfaces:** Add exported `BenchmarkExecutionError::{Cancelled,WorkerFailure,CleanupRequired}` and `CompletedBenchmark { target_path: PathBuf, xml: String }`. Crate-private `execute_lease(lease: ScratchLease, run: impl FnOnce(&ScratchLease) -> Result<ProcessOutput, ProcessError>) -> Result<CompletedBenchmark, BenchmarkExecutionError>` governs the journal/process lifecycle. Add crate-private `ScratchLease::retain_for_recovery(&mut self)`; explicit cleanup remains separate from Drop. Test-only journal/root constructors cannot be enabled in production builds.

- [ ] Make the existing four execution tests compile, then observe behavior failures: durable `running` before child launch; ordinary completion cleans after drain; drained cancel cleans before reporting; undrained/panic retains file and journal. Add native-I/O-error and nonzero-exit cases, asserting no false completion.
- [ ] Implement conservative Drop suppression before potentially uncertain I/O/launch. Only paths with positive stopped-I/O/job-empty proof call explicit cleanup; uncertain errors preserve the exact recovery record. Never map a kill request directly to proof of drain.
- [ ] Complete real temporary-file journal tests: restart recovery of the captured file; repeat recovery; replacement, link, reparse, wrong size/ACL, partial Prepared identity and unavailable volume cause zero unrelated deletion. Keep production volume-root checks unchanged; use an explicit test-only policy for temporary roots.
- [ ] Run `cargo test --locked -p partition11-benchmark-worker --lib`, `cargo fmt --all -- --check`, and strict targeted Clippy. Do not invoke ignored device/DiskSpd fixtures locally.
- [ ] Commit the recovery/worker slice, push to the existing feature branch and run the revised pinned-engine compatibility test through PR #53. Pin this workflow to Rust 1.99.1 instead of floating `stable`. Require valid XML, confirmed Job drain, file absence and empty journal in CI.

**Exit / Gate A:** All targeted tests pass and a fresh Windows CI run proves the exact ordinary-handle engine sharing/cleanup design. A red compatibility result blocks production connection; do not patch the engine or restore delete-on-close.

## Task 2 — Connect authorization, preparation and the one-run agent

**Files:** Create `crates/windows-agent/src/benchmark_run.rs`; modify `benchmark_protocol.rs`, `main.rs`, and existing PnP/scan modules only as required. Add `crates/benchmark-worker/src/preparation.rs` and exports in `lib.rs`.

**Interfaces:** `read_request_frame(reader: &mut impl BufRead) -> Result<BenchmarkRequest, RequestError>` consumes at most the initial 2-KiB newline frame. Add `run_file_benchmark(root: &Path, engine: &VerifiedDiskSpd, profile: DiskSpdProfile, direction: DiskSpdDirection, cancel: &AtomicBool) -> Result<CompletedBenchmark, BenchmarkExecutionError>`. `prepare_scratch(lease: &mut ScratchLease, cancel: &AtomicBool, deadline: Instant) -> io::Result<()>` initializes and flushes the exact file. `benchmark_run::run<R: BufRead + Send + 'static>(reader: R, writer: &mut impl Write)` owns the input reader, allowing its control thread to own that reader without borrowing an expiring stdin lock. It is the fixed dispatch entry and emits the addendum's final envelope.

- [ ] Add failing tests for bad HMAC, stale request/volume binding, ineligible filesystem, low free space, PnP revision change and expired scan: each asserts zero scratch creation. Add framing tests with a still-open input pipe, oversized first line, cancel/EOF after the request and malformed control bytes.
- [ ] Add preparation tests on small disposable leases: complete initialized content/length, bounded writes, flush before engine launch, cancellation/deadline, late I/O and unsupported file attributes. Use injectable clock/I/O only where timing or error cannot be deterministic; never skip the real file identity tests.
- [ ] Implement the fixed protocol and packaged-engine resolver. Validate authorization before scan, reject elevated execution, rescan after consent, compare keyed identity and current eligibility/free space, and retain a PnP generation guard through preparation/run/cleanup. Never pass an arbitrary renderer path to a native API.
- [ ] Implement proposed preparation with durable journal evidence before writing; persist `running` immediately before contained engine launch. Keep the same held verified engine handle until process completion. Decode only bounded UTF-8 XML; success requires zero exit and confirmed cleanup.
- [ ] Enforce 120-second agent watchdog and the phase budgets from the addendum. Disconnect/cancel/PnP invalidates success even if it races normal completion. Stop/join auxiliary observers before returning; do not join a thread that can block forever reading control input.
- [ ] Run `cargo test --locked -p partition11-agent` and worker tests, strict Clippy and locked Windows release build. Keep both live-device tests ignored and runtime false. Commit the complete agent slice.

**Exit:** Synthetic scan/temporary-file integration proves authorization -> preparation -> process -> cleanup. Malformed or stale requests have no file side effects; real inventory is not exercised locally.

## Task 3 — Complete main transport, consent, XML and recovery serialization

**Files:** Create `src/main/benchmarkAgent.ts` and `benchmarkAgent.test.ts`; modify `benchmarkRunController.ts`, `benchmarkRecovery.ts`, their tests, `main.ts`, `benchmarkPolicy.ts`, `src/shared/benchmark.ts`, and `package.json` test registration. Reuse `benchmarkXml.ts` and its fixtures.

**Interfaces:** `BenchmarkAgentService(executablePath: string, options?: BenchmarkAgentOptions)` with `execute(request: BenchmarkAuthorizationEnvelope, signal: AbortSignal): Promise<BenchmarkWorkerResult>`; options permit test spawn/clock injection only, never renderer configuration. Preserve renderer start/cancel request keys. Extend `BenchmarkWorkerResult` and its controller validator with exact `{ status: "stale_selection" }`; map the corresponding authenticated agent error to it, and malformed transport to the existing sanitized failure contract. Extend `BenchmarkConsentDetails` with `preparationWriteBytes`, `maxOperationSeconds` and an explicitly defined total-write disclosure. Availability gets an exact `preview | ready` discriminated union without stable identities/paths.

- [ ] Add failing transport tests for spawn failure, partial multibyte UTF-8, 8-MiB response/16-KiB stderr caps, duplicate/trailing final messages, wrong request ID, nonzero exit, abort before launch, post-abort success, `exit` before `close`, and a kill that does not close the child. Assert one live/quarantined process slot until close.
- [ ] Add consent tests for a 1-GiB preparation write in both directions, all native timeout allowances and throttle/in-flight allowances. Derive a documented upper bound from the pinned engine before claiming a maximum. Arithmetic parity fixtures run in Rust and TypeScript. Preserve explicit per-run consent; decline creates no file or child.
- [ ] Implement direct fixed spawn (`shell: false`, hidden window), bounded framed stdin with live cancellation, strict final parsing after `close`, phase/main deadline escalation and quarantine. Replace the main `execute` stub; do not set the build gate true.
- [ ] Feed completed XML through `parseDiskSpdXml` with the exact owned path, pinned version, direction/profile and 15-second duration. Construct the internal report from validated values; the controller rebinds the identity and strips it before returning the one-use token/report. Malformed engine output produces no report.
- [ ] Refactor recovery behind the same lifecycle coordinator. Test recovery timeout followed by late `close`, recovery refresh during a run, repeated recovery, deferred volume and latch reset; uncertain cleanup must block the next start. Add an injectable spawn seam to test actual recovery lifecycle instead of parser-only tests.
- [ ] Run `npm run test:main` and `npm run build`; add all new tests to the explicit test list. Commit the complete main slice.

**Exit:** Mock child streams exercise the full main controller/transport/XML chain, not only a fabricated worker report. No path, HMAC key/hash, native error or raw XML reaches renderer/logs.

## Task 4 — Make real-target benchmark UI functional

**Files:** `src/renderer/{App.tsx,BenchmarkPanel.tsx,benchmark.css,benchmarkPresentation.ts}`, `src/main/preload.ts`, `src/shared/protocol.ts`, `tests/windows-ui-smoke.cjs`, relevant main/presentation contract tests.

**Interfaces:** Feed `BenchmarkPanel` a real eligible `ReadOnlyVolume` and its current opaque selection token from inventory state; retain an explicit separate demo view. Consume the existing `startBenchmark`/`cancelBenchmark` bridge methods and the new availability union. No renderer target-path or native-engine options.

- [ ] Add failing mocked UI scenarios: no target, ineligible/stale target, preview mode, consent decline, ready start, preparing/running/cancelling, successful measurement, cleanup-required and stale cancel. Refresh invalidates old tokens and clears the previous target's active presentation.
- [ ] Implement explicit inventory-to-benchmark selection, one profile/direction per run, fresh native consent, active request ID, Start/Cancel behavior and sanitized error/progress states. Read/write result cells remain associated with the selected token; demo values never masquerade as measured results.
- [ ] Replace outdated blanket safety text only where benchmark capability is actually ready. State unsupported format/repair/resize/mount clearly. Add accessible labels/status announcements, focus return after dialogs, keyboard start/cancel and progress updates.
- [ ] Run `npm run build`, `npm run test:main` and mock-only `npm run test:ui:win`. Require real rendered Electron behavior; capture-disabled local success cannot count as screenshot evidence. Commit the UI slice.

**Exit:** The compiled UI completes ready/error/cancel flows through mocked privileged boundaries; no physical device is enumerated or benchmarked by smoke tests.

## Task 5 — Prove whole-operation failures and preserve existing features

**Files:** Worker/agent integration tests, `benchmarkAgent.test.ts`, `benchmarkRecovery.test.ts`, `tests/windows-ui-smoke.cjs`, `tests/run-safe-diskspd-ci.cjs` and `.github/workflows/windows-diskspd-xml.yml`.

**Interfaces:** The production pipeline remains unchanged; tests inject only scan/clock/process seams or use `#[cfg(test)]` temporary-root APIs. No release binary accepts test path overrides.

- [ ] Add stateful temporary-file fixtures for preparation failure, cancellation during every phase, primary exit with surviving descendant, output flooding, agent crash/restart, main disconnect, PnP change and replacement before cleanup. Verify no early file deletion, no lost journal, no post-cancel report and no parallel second run.
- [ ] Exercise the exact pinned engine in CI temporary storage for all four profiles and both directions with CI-only small bounded files/durations. Validate engine arguments, cap configuration, XML binding and cleanup; add a cancel case using the real production supervisor. Never report CI throughput as hardware acceptance.
- [ ] Explicitly invoke only the harmless process-tree fixture and pinned-engine CI fixture; never use a blanket `--ignored` command that might run physical-device probes. Replace obsolete delete-on-close expectations with approved restart-recovery expectations while preserving the error-32 regression test.
- [ ] Regress P2 metadata parser/identity fixtures, P3 independent ext-image listing/export/no-overwrite/source-hash checks, existing fuzz smoke, IPC isolation and raw-write capability denial. Preserve P3 host-process-crash staging policy as an explicitly open separate design item; do not delete unknown export stages or claim crash recovery is complete.
- [ ] Run the full locked Rust workspace, format, strict Clippy, release build, main/build/UI checks and current npm/Rust audits. Production npm findings must be zero; review existing Moderate dev-only advisory paths, no blind downgrade/audit fix. Regenerate target-specific SBOM with benchmark-worker dependency and verify graph/licenses.
- [ ] Push a runtime-disabled integration commit and require fresh CI, UI and DiskSpd checks on that exact SHA. Commit test/integration fixes with fresh evidence.

**Exit / Gate B:** Every automated safety path passes on the runtime-disabled integrated code. Any unresolved safety/recovery/measurement finding blocks activation.

## Task 6 — Review and activate the test-candidate capability

**Files:** Main build gate, availability contract/policy, CI assertions and `docs/{PLAN.md,SESSION_HANDOFF.md,P2_5_DISK_BENCHMARK.md}`.

- [ ] Obtain one fresh whole-branch review covering approval/spec coverage, native unsafe/handle lifetime, write-impact proof, recovery/quarantine, renderer trust and final artifact tests. Resolve actionable findings and rerun only affected checks before activation.
- [ ] Verify Gate A/B evidence references actual commits and has no missing or skipped required job. Correct stale draft PR #53 title/body/status for the final implementation, keeping it draft and unmerged.
- [ ] In a separate reviewed commit, enable file-backed benchmark readiness for the test candidate only. Main still requires clean recovery, verified pinned resources, supported platform and a fresh eligible token. Raw/image-write capabilities remain false; no environment/renderer override.
- [ ] Run all required workflows on the activation commit, including unsupported/unavailable mode, mock ready mode and package checks. Record actual source SHA and workflow IDs; a failed gate leaves the candidate unshippable and requires a corrective commit.

**Exit / Gate C:** CI is green on the exact activated candidate commit, with no unresolved review findings. Hardware acceptance is still pending and not used to claim this gate passed.

## Task 7 — Produce and verify the complete handoff package

**Files:** `package.json`, `.github/workflows/windows-ui.yml`, new `scripts/verify-windows-candidate.ps1`, new `tests/windows-package-smoke.cjs`, `docs/SESSION_HANDOFF.md`, candidate manifest and hash sidecars under `release/`.

**Interface:** `verify-windows-candidate.ps1 -PackageRoot <unpacked-root> -ExpectedCommit <sha>` verifies packaged resources/hashes and the generated manifest without querying disks. `windows-package-smoke.cjs <unpacked-root>` opens the packaged renderer/compiled preload with harness-owned mock IPC; it must not add a production mock switch.

- [ ] Add package verifier failures for missing/wrong agent, duplicate/wrong engine, absent MIT notice, stale agent SBOM/dependency graph, source SHA mismatch and leaked test binaries. Test the verifier on disposable copies with one resource changed at a time.
- [ ] Build the unsigned x64 candidate in CI from Gate C's commit; include the real agent, exact DiskSpd, MIT notices, Node/agent/engine SBOMs and provenance manifest. If packaging locally, back up/restore the existing staged agent rather than silently replacing unrelated developer resources.
- [ ] Smoke the packaged application in an isolated user-data directory, mock privileged IPC in the external harness, and test actual packaged resource paths/capabilities plus no-overwrite image fixture export. Do not scan physical volumes, create a benchmark file on a user volume or invoke DiskSpd locally.
- [ ] ZIP the complete app tree, reopen and read every entry to verify archive integrity, compare extracted file hashes against the manifest and record byte sizes. Do not distribute the standalone EXE without its Electron/resources dependencies. CI and local ZIPs are distinct builds with distinct hashes, not assumed byte-identical.
- [ ] Deliver ZIP + `.sha256` + manifest + evidence summary + user Windows 11 checklist. Record EXE/agent/engine/SBOM hashes and source/workflow IDs. Checklist covers USB/NVMe/SATA, denial/cancel/hotplug, protected/BitLocker/unsupported targets, crash/restart cleanup, DPI/Narrator/high contrast; the user runs it.

**Exit / Gate D:** A downloadable full ZIP exists, all entries/resources verify, packaged smoke passes, and only explicitly listed human/device gates and later roadmap work remain. No claim of physical acceptance, signing or production release.

## Final verification commands

Run with the pinned Windows toolchain after the final code change and retain exit codes/counts:

```text
npm ci
npm run build
npm run test:main
npm run test:ui:win
npm audit --omit=dev --audit-level=moderate
npm audit --audit-level=high
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --release
cargo audit --file Cargo.lock
cargo audit --file fuzz/Cargo.lock
```

In addition: the explicit CI-only engine/process fixtures, SDK ABI oracle, ext-image fuzz/oracle checks, packaged application smoke, candidate manifest/SBOM verifier and ZIP integrity pass. Report counts from fresh output; historical counts are context only. Completion means Gate A + B + C + D, with Windows 11 human acceptance clearly pending.

## Planning self-review

Checked: every approved production-plan task maps to Tasks 1–7; ADR-0004 requirements map to Tasks 1/2/3/5; addendum decisions map to Tasks 2/3/6. The five review-focus cases each have an owning test task. Public renderer request keys remain unchanged; proposed native/transport interfaces are defined above. Existing staged RED tests are accounted for, not discarded. No implementation or external write is performed by this planning document.
