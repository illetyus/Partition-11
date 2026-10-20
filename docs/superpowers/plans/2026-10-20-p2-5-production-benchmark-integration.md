# P2.5 production benchmark integration and Windows x64 test candidate — implementation plan

> Status: Approved for implementation on 2026-10-20; implementation and verification are in progress.
> Worktree: `D:\Partition 11\.worktrees\readonly-disk-preview`
> Execution: sequential, test-first changes in the existing worktree; preserve all current work. The user later authorized the necessary commit, feature-branch push, and draft PR solely to trigger CI; do not merge or run physical-device tests.

## Goal

Connect the existing inert Benchmark screen to one bounded, consented DiskSpd run against a newly created application-owned scratch file on a freshly revalidated eligible volume. Validate the complete cancellation, cleanup, result, packaging, and CI paths, then produce an unsigned Windows x64 test ZIP with hashes and the required license/SBOM files.

This closes the P2.5 implementation slice and yields a candidate for the user's Windows 11 acceptance. It does not claim that physical Windows 11 acceptance, mixed-DPI/Narrator acceptance, signing, or production release has passed.

## Approved safety boundaries

- Renderer requests contain only a one-use selection token, fixed profile ID, direction, and request ID. Main owns consent, bounds, identity binding, arguments, and result validation.
- Every run requires fresh main-process consent. Write consent is per run; no remembered consent.
- The agent rescans and recomputes keyed volume identity after consent and immediately before creating the scratch file.
- DiskSpd is the exact pinned Microsoft v2.3 AMD64 executable. Its extracted executable hash is pinned in addition to the existing archive hash. No runtime download, shell, arbitrary switches, paths, targets, or elevated process.
- DiskSpd receives one exact scratch-file target, never a raw disk, partition, or volume device. It cannot create or truncate the lease itself.
- Runtime, output, XML, process count, write rate, write-byte estimate, scratch size, and cleanup are bounded. Cancellation/timeout must empty the Job Object before cleanup or success is reported.
- Unsupported filesystem behavior, identity drift, stale/replayed token, PnP change, hash mismatch, invalid output, escaped child, or uncertain cleanup fails closed and disables another run.
- UI controls and runtime capability remain disabled until all P2.5 gates in this plan pass. Formatting, repair, resize, mount, raw I/O, elevation, and physical writes stay disabled.
- Automated validation uses fixtures, mocks, and disposable temporary files only. Do not enumerate/query physical devices for acceptance and do not run DiskSpd or any write test against physical media. The user performs physical Windows 11 acceptance.

## Task 1 — Prove the lease lifecycle before connecting the worker

Files:
- Modify: `crates/benchmark-worker/src/scratch.rs`
- Modify: `crates/benchmark-worker/src/lib.rs`
- Test: Windows-only benchmark-worker tests
- Document: `docs/P2_5_DISK_BENCHMARK.md`

Steps:
1. Add failing tests for lease sharing, delete-on-close behavior, exact file identity, replacement/reparse rejection, cleanup after ordinary completion, and cleanup after validation errors. Tests must use a disposable temporary directory.
2. In Windows CI only, run the exact pinned DiskSpd binary against a disposable temporary-file lease to prove it can open/use the lease under the intended sharing and delete-on-close flags, and that the file disappears after all handles close. Do not run this probe locally against a mounted user volume. Capture the Windows/filesystem used and the exact outcome.
3. Implement the least-privilege lease ACL/share flags and delete-on-close lifecycle only if the CI feasibility test proves DiskSpd compatibility. Retain handle-based file-ID/volume binding and exact-handle cleanup.
4. If delete-on-close is incompatible, stop integration and keep benchmark unavailable. Revise the approved design with an exact-object crash-recovery journal (protected for the current Windows user, identity/file-ID/size checked, no wildcard cleanup), then obtain written spec/plan approval before implementing that alternative.
5. Run targeted worker tests and formatting/Clippy. Confirm no cleanup path can delete a different file or report success while the object remains.

## Task 2 — Pin and package the engine and fixed workload

Files:
- Modify: `third_party/diskspd/manifest.json`
- Modify: `crates/benchmark-worker/src/diskspd.rs`
- Modify: `crates/benchmark-worker/src/lib.rs`
- Test: worker unit tests and `tests/diskspd-manifest.test.cjs`

Steps:
1. Add failing tests for the extracted executable SHA-256, unexpected architecture/version/size, reparse/replacement cases, fixed argument vectors, raw target rejection, and prohibition of DiskSpd create/truncate switches.
2. Compute and review the executable SHA from the pinned archive in CI; record it in the manifest. Keep the existing upstream archive hash and provenance.
3. Implement verification through the already-open verified handle and expose only fixed workload constructors. Enforce one thread, one target, one repetition, approved profile/direction, fixed warmup/duration, and the single shared write-cap/byte-estimate conversion.
4. Keep paths and switches private to the worker. Verify read/write policies cannot be changed by renderer-controlled values.
5. Run Rust tests, manifest tests, Clippy, and the CI extraction/hash check.

## Task 3 — Add the one-run agent protocol and fresh target revalidation

Files:
- Create/modify: `crates/windows-agent/src/benchmark_protocol.rs`, `benchmark_run.rs`, and dispatch in `crates/windows-agent/src/main.rs`
- Reuse: current inventory, volume eligibility, keyed storage identity, ScratchLease, and Job Object supervisor
- Test: agent protocol/worker tests with injected scan and filesystem seams

Steps:
1. Add failing protocol tests for exact required fields, unknown/duplicate fields, bounds, malformed frames, stale/replayed request IDs, bad HMAC, and no path/device identity disclosure.
2. Add a separately versioned benchmark-worker mode over bounded stdin/stdout; leave inventory protocol versioning independent.
3. Accept only the one-run authorization envelope. Re-scan after main consent, recompute the keyed identity, verify the one-use target still maps uniquely to the same eligible volume, check PnP generation and current free space, then create the lease.
4. Create the scratch file only after every authorization and eligibility check succeeds. Reject malformed requests before side effects.
5. Pass only the canonical lease file target and the fixed verified engine/arguments into the existing Job Object supervisor. Never open a physical volume or raw target for writing.
6. Add injected tests proving denial/staleness/identity change causes zero file creation and that PnP invalidation cancels or rejects the run.
7. Run locked agent/worker tests, Clippy, and Windows release build. No live-device inventory probe.

## Task 4 — Implement main-process consent, token consumption, and bounded IPC

Files:
- Modify: `src/main/main.ts`, `src/main/benchmarkPolicy.ts`, `src/main/preload.ts`, `src/shared/protocol.ts`, `src/shared/benchmark.ts`
- Test: `src/main/benchmark-preview.test.ts` and new focused IPC/policy tests

Steps:
1. Add failing tests for untrusted-frame rejection, one-use/expired token rejection, concurrent-run rejection, missing or declined consent, per-run write consent, and renderer attempts to submit paths, executable, switches, durations, repetitions, XML, or identities.
2. Main consumes the selection token once, verifies the selected profile/direction against the allowlist, calculates and presents scratch size, maximum duration, write-rate limit, and conservative write-byte upper bound from fixed worker inputs.
3. Show a native consent dialog bound to the selected volume label/mount/filesystem and exact run parameters. Decline must result in no child process and no scratch file.
4. Main starts only the fixed packaged Rust agent directly (no shell), sends a bounded versioned envelope and ephemeral HMAC key over stdin, and accepts only bounded status/final-result frames.
5. Add exactly one active-run state machine. Refresh, denial, cancellation, timeout, completion, window close, or shutdown invalidates the token and prevents duplicate launch.
6. Keep logs and renderer payloads free of stable identifiers, local paths, raw XML, engine paths, and native error details.
7. Run targeted main tests and TypeScript build; verify capability remains false until all later gates pass.

## Task 5 — Cancellation, shutdown, and exact cleanup integration

Files:
- Modify: `crates/benchmark-worker/src/job_process.rs`, `scratch_lease.rs`, `crates/windows-agent/src/benchmark_run.rs`, `src/main/main.ts`
- Test: worker, agent, and main cancellation tests

Steps:
1. Add failing tests for cancel before launch, cancel during run, timeout, window close, shutdown, cancellation racing normal exit, child output flooding, and an undrained descendant.
2. On every abort path, terminate the complete Job Object and wait within the fixed grace. Do not release/delete the lease until the job is empty.
3. Delete by held handle and verify the exact file ID is gone. If draining or deletion is uncertain, return a sanitized failure, retain recovery state if the approved design requires it, and block subsequent runs.
4. Make cancel idempotent; stale cancel IDs cannot affect another run.
5. Run targeted tests, then the full Windows worker suite. No benchmark against a user volume.

## Task 6 — Validate DiskSpd XML and expose sanitized measurements

Files:
- Modify: `src/main/benchmarkXml.ts`, `src/main/main.ts`, `src/shared/benchmark.ts`
- Test: strict parser and main result tests

Steps:
1. Add failing tests for DTD/entities, malformed/oversized XML, extra targets/spans/threads, inconsistent counts, wrong profile/direction/target/version, stale request IDs, nonzero exit, post-cancel output, and invalid timing.
2. Require one final XML result bound to the active request, exact profile/direction, scratch target, engine version, validated duration/counts, and successful process exit.
3. Display only validated measurements and safe metadata: profile, direction, decimal MB/s, IOPS, available latency percentiles, engine version, filesystem, scratch size, cache policy, duration, and timestamp.
4. Keep demo data clearly separate. Never pass raw XML, target path, identity hash, serial, PnP ID, or native error to the renderer.
5. Run parser/main tests and verify rejection leaves no success-shaped result.

## Task 7 — Enable the UI only after all gates and verify the interaction

Files:
- Modify: `src/renderer/BenchmarkPanel.tsx`, `src/renderer/App.tsx`, `src/renderer/benchmark.css`, `src/main/preload.ts`
- Test: `tests/windows-ui-smoke.cjs` and renderer/main tests

Steps:
1. Add failing smoke assertions for Start disabled when unavailable, missing/stale selection, and run-in-progress state; Cancel is available only for the matching active run.
2. Show consent and progress state from main-owned data, not renderer-generated workload details. Show sanitized errors and cleanup-required status clearly.
3. Enable Start only when the capability handshake confirms every P2.5 safety gate and a currently valid selection. A failed gate always leaves it disabled.
4. Add keyboard/focus/accessible-name coverage and ensure no mock result is presented as a live benchmark.
5. Run Windows Electron smoke with mocked inventory/IPC only. Do not launch against real disks or capture claims of physical acceptance.

## Task 8 — CI, license/SBOM, and reproducible x64 candidate

Files:
- Modify: `.github/workflows/windows-diskspd-xml.yml`, `.github/workflows/windows-ui.yml`, `.github/workflows/ci.yml`
- Modify: `package.json`, `third_party/diskspd/manifest.json`, `docs/LICENSES.md`
- Test: `tests/run-safe-diskspd-ci.cjs`, packaging/manifest/SBOM checks

Steps:
1. Add failing CI checks that the pinned archive and executable hashes match, exactly one engine is packaged, the MIT notice is present, and the CycloneDX SBOM records DiskSpd v2.3 source, AMD64, license, and executable hash.
2. Package the benchmark worker, fixed agent, verified engine, MIT notice, and SBOM as explicit resources. Do not include developer-only binaries or runtime download code.
3. Extend Windows CI to run the real pinned engine only against a disposable temporary file and validate XML, fixed arguments, write cap, cancellation, process drain, and exact cleanup. No host physical-device inventory or benchmark.
4. Build the unsigned Windows x64 candidate in CI and locally if prerequisites permit. Record app/agent/engine/notice/SBOM hashes and the ZIP SHA-256 in a sidecar and candidate manifest.
5. Run package extraction/CRC checks and launch the packaged app in the existing mock-only UI smoke. State clearly that unsigned preview and CI temporary-file success do not equal physical Windows 11, mixed-DPI, Narrator, or release acceptance.

## Task 9 — Close the candidate handoff and report remaining user gates

Files:
- Update: `docs/P2_5_DISK_BENCHMARK.md`, `docs/PLAN.md`, `docs/SESSION_HANDOFF.md`, and candidate manifest
- Test: final verification commands below

Steps:
1. Update status only from fresh evidence: mark P2.5 code/CI gates complete only after all preceding task checks pass; do not mark user hardware acceptance complete.
2. Record branch/commit state, the unsigned ZIP path and SHA-256, source/build versions, included resources, tests and known limitations.
3. Keep P4–P8 capabilities (image writes, physical formatting, repair, resize, mount) disabled; they are later, separately gated roadmap work and are excluded from this approved P2.5 candidate.
4. Provide the user a concise Windows 11 acceptance checklist for USB/NVMe/SATA, supported filesystems, BitLocker, hotplug, cancel, consent denial, cleanup, mixed-DPI, Narrator, and high-contrast. Do not execute those physical tests.

## Final verification

Run after the last code change; capture exact exit status and test counts:

- `npm ci`
- `npm audit --omit=dev --audit-level=moderate` (must show zero production findings)
- `npm audit --audit-level=high` (currently expected to retain the documented Moderate development-only GHSA-hp3w-g68c-fv3c path; review its output, do not auto-apply the dry-run downgrade)
- `npm run build`
- `npm run test:main`
- `npm run test:ui:win`
- `cargo fmt --all -- --check`
- `cargo test --locked --workspace` (never enable ignored live-device tests)
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace --release`
- `cargo audit --file Cargo.lock` and `cargo audit --file fuzz/Cargo.lock` and the pinned target SBOM validation
- Windows CI DiskSpd test using only a disposable file
- x64 ZIP integrity, executable launch, resource/hash/license/SBOM checks

No task is complete based solely on source inspection or local build output. Hardware, mixed-DPI, Narrator/high-contrast and physical-volume acceptance remain explicitly user-owned gates. No physical disk tests are run by the implementer.

## Completion boundary

This plan ends with a verified, unsigned Windows x64 candidate that implements consented file-backed benchmarking while keeping all other write/destructive capabilities disabled. It does not sign, publish, merge, or perform the user's physical-device acceptance. The user authorized a feature-branch push and draft PR only to run the required CI.
