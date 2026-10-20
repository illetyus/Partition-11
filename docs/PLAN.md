# Partition 11 implementation plan
## Fresh P3/P9 local verification - 2026-10-20

The 601-second Windows x64 ext_image fuzz campaign completed successfully: 10,091 runs, 1,306 coverage counters, 3,016 features, 138 new corpus units, 1,034 MiB peak RSS, and no crash or timeout. The isolated corpus contained fixture copies only and was removed after the run.

The actual x64 agent image worker and the compiled Electron main ReadOnlyImageInspectionService were exercised against clean synthetic ext4 fixtures. Both listed the known root and exported the expected 36 bytes; both rejected destination overwrite without leaving a stage file. The source fixture hash remained unchanged. This verifies main-to-agent image inspection, not the rendered UI.

The target-specific CycloneDX 1.5 agent SBOM contains 11 components. Running the Windows CI PowerShell assertions locally passed the required component versions, MIT license expressions, and dependency edges from agent to partition11-ext-reader to rust-fs-ext4 v0.8.0. docs/LICENSES.md and docs/P0_REPRODUCIBLE_BUILDS.md describe the refreshed graph.

This branch still has no GitHub Actions result. The Windows CI-only DiskSpd/delete-on-close gate remains open, so P2.5 production integration and runtime enablement are incomplete. No DiskSpd process, physical inventory, or physical-volume test was run.

Task 1 local regression coverage now includes junction-root rejection, exact-file replacement rejection, and the file reparse metadata flag. The locked worker suite passed 28 with 3 ignored; the full workspace passed agent 122, worker 28, core 20, and ext-reader 16. Workspace format, warning-denying Clippy, and release build passed.

An explicit process-exit cleanup probe returned the expected RED because the current lease is not delete-on-close; it remains ignored until the required Windows CI DiskSpd compatibility result. Production runtime stays disabled; no DiskSpd process or physical-device test ran.

## Current local checkpoint — P2.5 integration preparation (2026-10-20)

The user-approved P2.5 specification and implementation plan are in progress
in `D:\Partition 11\.worktrees\readonly-disk-preview`. Local work adds the
strict agent benchmark request parser, matching Rust/TypeScript HMAC encoding,
side-effect-free fresh-target validator, and Electron single-run consent
controller. Fresh Windows volume records now retain the validated internal
volume-GUID root; target revalidation requires its canonical form and returns it
as `volume_guid_root` for later lease creation. The public inventory serializer
continues to omit this internal path. Main validates exact worker result/report
fields, pinned DiskSpd version, target identity, fixed workload, and measurement
bounds; it strips the worker HMAC and returns only the one-use selection token.
The production runtime switch is deliberately `false`; Start/Cancel stay
disabled, and no worker, DiskSpd process, or scratch file is launched through
the UI.

Fresh verification: Electron main tests 76/76; `npm run build` passed; and
`PARTITION11_UI_SMOKE_SKIP_CAPTURE=1 npm run test:ui:win` passed 42 mock-IPC
assertions. The normal screenshot path returned local compositor
`UnknownVizError`, so no screenshot evidence is claimed. Fresh safe Windows
Rust checks passed: benchmark-worker tests 24 passed / 2 CI or process-fixture
tests ignored, core 20 passed, ext-reader 16 passed, workspace fmt,
warning-denying Clippy, and locked workspace release build. The full agent test
binary passed 122 fixture-only tests; its two physical-device probes remained
ignored. Fresh benchmark protocol tests passed 8/8, inventory serialization
tests passed 6/6, and warning-denying agent Clippy plus Rust format checks
passed. The pinned DiskSpd delete-on-close compatibility
test has not run in Windows Actions, so the approved plan's first gate remains
open. Production scratch ACL/lifecycle, agent dispatch, process cancellation/
cleanup, native XML-to-report integration, active UI, and a fresh unsigned
candidate are incomplete. The Windows CI-only DiskSpd/delete-on-close gate
remains open, so the production worker is still disconnected. No physical-
device test, inventory query, DiskSpd workload, or volume write was performed.
No commit, push, PR, or merge was made.

## Latest merged checkpoint — P2.5B.3 partition protection metadata (2026-10-19)

PR #51 merged as `eb2d932abdb492a040232fbbc3bc81ae558ce467` after 5/5 PR
checks. Fresh `main` at the same SHA passed CI, Windows Electron UI preview,
and DiskSpd XML integration 3/3:
[CI](https://github.com/illetyus/Partition-11/actions/runs/39309441359),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39309441521),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39309441451).

The Windows layout parser preserves MBR partition type/active indicator and
GPT type GUID fields through metadata worker protocol v2. Service, EFI,
Microsoft Reserved, Microsoft Recovery, active MBR, MBR recovery, and MBR EFI
partition classes can now be recognized by later benchmark policy. The slice
does not map volumes to disks, fully identify Windows system/boot volumes,
authorize targets, or launch DiskSpd. No physical disk tests were run.

Next: add trusted volume-to-partition mapping and fail-closed eligibility
checks for system/boot/recovery exclusions, supported local filesystem and
drive types, identity, and free space. Production benchmark launch remains
disabled until file ownership, consent, pinned engine, and cancellation gates
are implemented.

## Current local checkpoint — P0 dependency audit hardening (2026-10-20)

The production npm dependency graph has no known advisories. The full npm graph
reports eight Moderate records for the same dev-only `sprintf-js` advisory
through `electron-builder`; the upstream advisory lists no patched version.
The npm auto-fix dry run proposes downgrading `electron-builder` 26.15.3 to
26.5.0, so that unverified packaging downgrade was not applied. The temporary
acceptance and 2027-01-20 review deadline are documented in
`docs/SECURITY_DEPENDENCY_EXCEPTIONS.md`.

Local CI configuration now gates production npm advisories at Moderate and
the full npm graph at High/Critical. A pinned RustSec `cargo-audit` job is
configured to scan the committed Cargo lockfile without generating it. The
local RustSec `cargo-audit` 0.22.2 run passed with no advisories; the lockfile
contained only local workspace packages at that checkpoint; the P3-I1 image-reader slice below later adds pinned upstream Rust dependencies. Production
`npm audit --omit=dev --audit-level=moderate` also passed with zero findings;
the full npm graph has the documented Moderate development-only exception.

The target-specific CycloneDX 1.5 command was validated using a temporary copy
of the Cargo workspace: its `partition11-agent` SBOM includes the local
`partition11-core` dependency for `x86_64-pc-windows-msvc`. The Windows preview
workflow now installs the pinned generator, validates and copies that SBOM
beside the packaged agent, and uploads it separately. These local workflow
edits remain unverified until GitHub Actions runs. No phase is marked complete
based only on local configuration or temporary-copy SBOM generation.

---

## Current local checkpoint — P2.5B.4–B.7 pre-hardware work (2026-10-20)

Work remains local in `feature/p2.5-volume-eligibility-policy`, based on the
locally fetched `origin/main` (branch is three commits ahead). The read-only
preflight maps a drive-letter volume through bounded local WMI/CIM data to one
verified disk and partition, rejects protected/system/boot/ambiguous targets,
and requires supported filesystem/type, writable status, at least 2 GiB free,
and a stable disk identity. No volume-device handle, raw-sector operation,
write, elevation, or live benchmark is used.

The private agent protocol is v3. The main process creates a per-service
32-byte secret and sends it only over the child stdin request. The Windows
agent computes HMAC-SHA256 over the canonical stable device identifiers and
returns only a 64-character keyed identity hash. The main process validates
that hash, binds it to a fresh selection token in memory, and strips the hash
from the public renderer inventory. Missing or shared stable identifiers fail
closed. This is a process-local privacy binding, not executable authentication
or durable identity across app runs.

Review hardening now also rejects a candidate volume as `identity_unavailable`
if any scanned disk has unavailable or unreadable identity, because uniqueness
cannot be established from a partial set. The internal P2.5B.7 worker primitive
creates a suspended child inside its kill-on-close Job Object with
`PROC_THREAD_ATTRIBUTE_JOB_LIST`; it bounds output, deadline and cancellation,
and drains the process tree. The primitive is still unconnected to production
IPC or UI, and DiskSpd invocation remains disabled.

P2.5B.5 adds a separate Windows-only library crate, crates/benchmark-worker, with an internal ScratchLease primitive. Its production entry point accepts only a mounted volume root, so scratch creation stays on a stable volume-GUID root path instead of an arbitrary directory whose ancestors could be redirected. It atomically creates one cryptographically random CREATE_NEW file directly at that root, captures the returned handle's volume serial and 128-bit file ID, rejects reparse points and multiply-linked files, verifies a bounded aligned size, and deletes only that exact file by handle. Validation failures after CREATE_NEW also attempt exact-handle deletion. No directory is created or removed. The lease is not a binary and is not packaged or connected to Electron, the agent, or DiskSpd; temporary-directory roots are available only to unit tests.

Windows 11 IoT Enterprise LTSC build 26100 x64 checks on 2026-10-20: Rust
agent 104 passed / 2 live-device tests ignored; benchmark-worker 15 passed /
1 process-tree fixture ignored; core 19 passed. Workspace Clippy and the
Windows x64 release build passed. TypeScript/renderer build and main tests
passed (44/44). A prior unfiltered Rust run accidentally invoked both the
SetupAPI enumeration test and the full metadata inventory probe; the latter
timed out after about six seconds. It performed no write or benchmark. The
test output cannot establish how many read-only metadata requests completed.
Both live-device tests are ignored by default. GitHub Actions no longer runs host disk inventory. The UI smoke uses static mock data; the dedicated DiskSpd workflow operates only on a disposable temporary file. The user has not yet run physical
Windows 11 acceptance.

A local unsigned/unpacked x64 candidate was built by electron-builder
26.15.3 with Electron 44.8.0, Node 24.21.0/npm 11.19.0, and Rust 1.99.1:
`release/Partition-11-Windows-x64-p2.5b4-identity-electron-builder-20261020.zip`
(176,440,032 bytes; 73 entries; ZIP CRC passed). SHA-256:
`82a3f6830b6baf2c4e6744703171173c73bb7ea30d9503a04e07cc4834b28911`.
The AMD64 executable hash is `1a572709ef7b4ec1ccf38184e3e40024168429fe1075f1354487674a8d42a4a1`;
app.asar hash is `441aee96932d7374032d566c9a3c62b0bf4f75b0dc15ee4e08db6e042f7556f6`;
agent hash is `b8d16536d0a32abb241bf4e5d9f0cd8b9c6929485a1f5083360e83a638e0ca94`.
The Electron launch includes only the XML parser as a production npm dependency;
React and Fluent UI remain build-time renderer dependencies. Moving those
renderer-only dependencies to `devDependencies` let the standard packager finish
its dependency scan. This is a local build result, not yet a fresh CI/release
reproducibility result.

No live volume inventory, DiskSpd run, or physical-device acceptance was
performed. Live GitHub state was refreshed through the connected read API on
2026-10-20: `main` is `130868dbaf60a3dfa71f62f5d5086c985d792cc6`, its CI,
Windows UI, and DiskSpd workflows passed 3/3, and there are no open PRs. The
local branch has no PR or merge. The prior Electron ZIP predates this isolated
crate and does not contain it. The user will perform physical Windows 11
acceptance. The native scratch primitive and Job Object process-tree supervisor
are implemented and locally verified, but production worker integration, ACL
and crash-recovery policy remain open, alongside explicit write consent,
validated live metrics, CI integration, and disposable-hardware QA. Benchmark
launch and disk writes remain disabled. P3–P9 remain open.

## Current local checkpoint � P2.5B write-impact disclosure and x64 candidate (2026-10-20)

The benchmark preview now displays a live upper-bound write-byte and runtime estimate based on repetition count. The renderer and workload planner share one policy for the 100 MB/s DiskSpd throttle, 5-second warmup, 15-second measurement and 15-second termination allowance. The UI exposes only the 1 GiB scratch size accepted by the native ScratchLease. This estimate is a disclosure aid, not user consent or evidence of a live write. Benchmark Start/Cancel remain disabled; explicit per-run consent, production worker integration, target binding, ACL/crash recovery, verified metrics, CI and the user's Windows 11 acceptance remain open.

`npm run build` passed; main-process tests passed 47/47. The updated Windows Electron smoke passed 36 behavioral assertions using mock IPC. Screenshot capture was skipped after this host's local Electron compositor returned `UnknownVizError`; the smoke requested all 11 captures, but none are claimed for this candidate. No real inventory scan or write workload was run in this step. The user remains responsible for physical Windows 11 acceptance.

New unsigned x64 preview: `release/Partition-11-Windows-x64-p2.5b8-impact-utf8-20261020.zip` (159,166,921 bytes; 74 entries; ZIP CRC passed). SHA-256: `5ee8c7c83c6533a06cffa6190e60ead14cd1352a5eb9f13cd910d87269171f54`. Sidecar: `.sha256`. Electron 44.8.0 and electron-builder 26.15.3. Packaged EXE SHA-256: `33eb30d249267198b8be7605fe3f2fecd7b255891b24b7dd84feeb5d22e5d9d0`; agent: `65de2fb1a31b0476ca57cc50a8da196f65abb4acbdacc15878af26567b5226ba`; app.asar: `36b0992189ae2a79be38555f2c7d639fdc9820b52aca9babda0d2d5365d950d0`; SBOM: `60d689a058293223f7c5dc9f3e96bf1672484f6349cafddebb8bb484157415f4`. The packaged agent's static `capabilities` command reports image inspection, physical-disk inspection and both write capabilities false. Existing release archives were left untouched.

## Current checkpoint — P2.5 safety-gate preparation (2026-10-20)

The pinned Microsoft DiskSpd v2.3 AMD64 executable, MIT notice, release
manifest, and component CycloneDX 1.5 record are now package inputs for the
unsigned preview. Windows Actions checks the release archive and executable
hashes and includes a contained, read-only compatibility probe against a
disposable delete-on-close file. That CI gate has not run on this local branch;
no DiskSpd process was launched locally.

Read-only volume-selection tokens now expire after 60 seconds and can be
consumed only once by the main-process service. The approved workload is one
profile run per consent; the repetition selector was removed. Local
verification passed: Rust workspace tests (agent 113, worker 24, core 20,
ext-reader 16; two agent hardware tests and two worker-only tests ignored),
warning-denying workspace Clippy, `npm run build`, and 64 main-process tests.
The mock-only Windows UI
smoke passed 42 assertions with screenshot capture disabled. The normal local
capture failed at Electron `capturePage()` with `UnknownVizError`; screenshots
are not claimed. Production benchmark launch and its remaining consent,
identity revalidation, cleanup, result, and Windows Actions gates remain open.
No physical disk inventory, DiskSpd workload, formatting or write test ran.
The preview is not yet a functional benchmark candidate. Current audit reruns
on 2026-10-20 found zero production npm vulnerabilities and only the documented
eight Moderate dev-only `sprintf-js` records in the full graph (no High/Critical);
RustSec scanning passed for 31 locked dependencies. The packaged DiskSpd
executable hash matches the manifest. The Windows CI compatibility gate has
not run, so production benchmark launch remains disabled.

---

## Current checkpoint — P3-I2 independent fixtures, fuzz smoke, and fresh Windows candidate (2026-10-20)

The pinned rust-fs-ext4 v0.8.0 read-only image flow is connected through a one-shot Windows agent worker and Electron main/UI token boundary. Ext4 uses independent clean and recovery-pending e2fsprogs 1.47.3 fixtures; tests verify known tree/content, reject dirty or corrupt images, and preserve the source bytes through a deterministic 128-case metadata mutation sweep. Agent export uses a unique sibling staging file, syncs and commits without replacing an existing destination, and preserves foreign staging collisions. Main timeout cleanup is covered. Abrupt host-process crash recovery remains open.

The local ext-reader suite passed 16 tests (5 unit, 3 dirty/corrupt/mutation, 1 e2fsprogs oracle, 7 read-only integration). An earlier Windows cargo-fuzz smoke passed 62 executions in 31 seconds with 1,205 coverage features. A later fixture-only run completed 185 seconds, 371 executions, 1,226 coverage counters, 1,897 features and 619 MiB peak RSS with no crash. Both use only temporary image files and a 16 MiB input cap; this remains regression fuzzing, not a long fuzz campaign. Workspace fmt, warning-denying Clippy, and locked tests passed: agent 113 passed / 2 physical-hardware tests ignored; benchmark-worker 15 passed / 1 process-tree fixture ignored; core 20 passed. npm run build passed, main tests passed 60/60, and mocked Windows UI smoke exited 0 with screenshot capture skipped. Fresh RustSec audit loaded 1,338 advisories and found no issue in 31 locked dependencies. Fresh npm audit found no production vulnerability and only the documented eight Moderate dev-only `sprintf-js` records; no High/Critical finding.

Fresh unsigned x64 candidate: release/Partition-11-Windows-x64-p3-final-20261020.zip (163,956,723 bytes; 74 entries decompressed successfully), SHA-256 53431df71ad04c4d8f7822bb761139afc80ab827bab83f3f28f9706479a4b699. It includes the x64 agent and validated target-specific CycloneDX 1.5 agent SBOM. Packaged agent capabilities report image and physical-disk inspection enabled, both write capabilities disabled. GitHub Actions for this branch have not run. No physical disk inventory, benchmark, or write was performed.

Next: complete the P2.5 production worker and explicit-consent integration without running a workload, keep formatting/repair/resize/mount disabled, finish current npm advisory review after user authorization, then verify the local branch through CI. The user performs physical Windows 11 acceptance.

---
## Current checkpoint — roadmap item 2 bounded metadata I/O complete

[PR #46](https://github.com/illetyus/Partition-11/pull/46) merged to
`main` as `a9cb53344ff560082096d728eb453af0532a7415` after five passing PR
checks. The same SHA passed all five fresh post-merge checks:
[CI](https://github.com/illetyus/Partition-11/actions/runs/39195989162),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39195989210),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39195989174).

Roadmap item **2 is complete at code/CI level**. Windows metadata queries now
use a disposable worker with 5-second overlapped request budgets, exact
`CancelIoEx`, a 1-second completion grace, independent 20-second worker and
parent limits, and post-worker generation/inventory validation. Timed-out,
cancelled, malformed or stale results are discarded. See
[P2_I2C_BOUNDED_IO.md](P2_I2C_BOUNDED_IO.md) for behavior and limits.

At this main-branch checkpoint, [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
still had durable hardware identity, physical Windows 11 USB/NVMe/SATA/
BitLocker/512e/4Kn acceptance, and authenticated read-only agent IPC/UI open.
The current worktree checkpoint above implements bounded sender-validated
IPC/UI locally but is not merged. The request ID is correlation only and does
not cryptographically authenticate the executable. No raw-sector operation,
write, format, mount, repair, resize, or elevation is enabled.

---

## Previous checkpoint — roadmap item 1 per-HANDLE removal lifecycle complete

[PR #43](https://github.com/illetyus/Partition-11/pull/43) merged as
`b17e0fbeacd9ee28f7383b5a75a1d7a8198f6a43` with **5/5 PR checks and
5/5 post-merge `main` checks successful**.

Roadmap item **1 is complete at code/CI level**. Agent-owned metadata HANDLEs
now receive `CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE` notifications. Query-remove
closes idle handles, but never closes underneath a synchronous IOCTL; busy
query-remove is vetoed and close is serviced after the call returns. Pending
and complete removal actions invalidate the scan and use the same safe
close/deferred-close state. Query-remove-failed is recovered only through a
fresh subsequent SetupAPI scan, not by reviving stale state. See
[P2_I2C_HANDLE_REMOVAL.md](P2_I2C_HANDLE_REMOVAL.md).

Post-merge evidence: [CI](https://github.com/illetyus/Partition-11/actions/runs/39179456003),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39179456177),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39179455971).

At the time of this checkpoint, item 2 was the next task. It is now complete
through [PR #46](https://github.com/illetyus/Partition-11/pull/46); see the
current checkpoint above. The hardware and authenticated-IPC gates remain.

---


## Previous checkpoint — P2-I2c-5 query-epoch rejection merged

[PR #41](https://github.com/illetyus/Partition-11/pull/41) merged to
`main` as `6b9f293dea427f598fdc35ed355d9d39f5d6d3dd` after 5/5 passing
PR checks. A short-lived agent-private `ProbeEpoch` now prevents starting
further metadata IOCTLs after a Windows PnP generation change, rejects
responses even if the call returned success, and checks each bounded
drive-layout retry. This is fail-closed stale-result rejection, **not**
forceful cancellation of a blocked synchronous Windows call.

At this historical checkpoint, [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
still called for handle-specific query-remove notifications and blocked-I/O
cancellation. PR #43 later added per-HANDLE removal handling, and PR #46
added bounded cancellation in a disposable worker. The remaining gates are
durable hardware identity, physical Windows 11 acceptance, and authenticated
read-only IPC/UI. See [query-epoch notes](P2_I2C_QUERY_EPOCH.md).
No Electron disk privileges or physical writes are enabled.
New `main` runs: [CI](https://github.com/illetyus/Partition-11/actions/runs/39176273055),
[UI](https://github.com/illetyus/Partition-11/actions/runs/39176273089),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39176273046);
verify final statuses separately.

---


Last reviewed: 2026-10-18. Application name is **Partition 11** and the
GitHub repository remains **illetyus/Partition-11** by user decision.

## Previous checkpoint — P2-I2c-3 cancellable scan generations merged

[PR #37](https://github.com/illetyus/Partition-11/pull/37) was merged
as `fd920434101ce5b7797caced9d0c451caf86dc2e` with 5/5 passing PR
checks. It provides a private atomic invalidation primitive and verified
tests for stale snapshot cancellation; non-elevated aggregate-only scans
invalidate their generation on normal completion. Windows Clippy checks
were repaired, not globally disabled.

At this historical checkpoint, PnP device-change notifications and
handle-specific cancellation were not yet implemented. PR #39 later added
PnP event invalidation, PR #43 added per-HANDLE removal handling, and PR #46
added bounded in-flight I/O cancellation. Durable hardware identity,
physical Windows 11 acceptance, and authenticated read-only IPC/UI remain in
[Issue #31](https://github.com/illetyus/Partition-11/issues/31). All disk
read/write GUI capabilities remain false.

## Objective

Build a Windows 11-oriented Electron app for safe ext2/ext3/ext4 partition
inspection and management. Avoid Linux/WSL as runtime dependencies.
Native storage access must be isolated from the Electron renderer.

## Non-goals for the first release

- Bypassing Windows volume locks or formatting the system/boot disk.
- Guaranteed repair of arbitrary corrupt ext4 volumes.
- Explorer drive-letter mounting or untested write support.
- Implementing ext4 algorithms from scratch without a conformance harness.
- Shipping GPL source code within the MIT-only app before compatibility review.

## Phases and release gates

| Phase | Deliverables | Hard gate |
|---|---|---|
| P0 Foundation | MIT skeleton; architecture, licensing, threat model, CI, Electron shell, Rust capability policies | Rust tests + TypeScript build |
| P1 Adaptive UI | Fluent UI 2, native Windows theme, responsive disk map, keyboard, accessibility, DPI matrix | 100/125/150/175/200/250/300% at mixed-DPI |
| P2 Read-only discovery | Native Win32 device enumeration, GPT/MBR parser, stable target identity, image inspection | No raw writes; golden layout fixtures |
| **P2.5 Disk Benchmark — NEW** | DiskSpd file-only worker, UI, bounded temporary test files, read/write workloads, XML metrics | No raw disk targets; ownership and cancellation; disposable-drive QA |
| P3 ext read-only | File listing and export for supported ext feature flags, no journal replay writes | Linux/e2fsprogs differential tests |
| P4 Image-only writes | mkfs, metadata edits, fault-injection tests, safe error reporting | Images only; e2fsck -fn; power-failure tests |
| P5 Physical-device formatting | Dedicated privileged agent, strict lock, UAC, explicit plans and hardware tests | Disposable hardware acceptance |
| P6 Repair | Separate fsck audit and limited repair; never claim full e2fsck equivalence | Per-repair-class verified oracle |
| P7 Resize | Distinguish GPT/MBR partition resizing from ext filesystem resizing | Both directions with failure rollback strategy |
| P8 Optional mount | WinFsp licensing and I/O validation; start read-only | Legal/security review + conformance |
| P9 Release | Signing, packaged installers, SBOM, privacy, updater policy | Reproducible CI, release provenance |

P2.5 runs after P2 volume discovery and before P3 ext browsing. File-only benchmarks do not authorize direct block writes or partition changes.
P6/P7 are conditional on a sufficiently capable engine; replace/port tooling
only after an explicit architecture decision and license review.

## Engine decision

- Primary **evaluation candidate**: [rust-fs-ext4](https://github.com/antimatter-studios/rust-fs-ext4)
  v0.8.0 (MIT). Its upstream feature matrix documents partial fsck and NO resize.
  It is NOT yet approved as a production write engine.
- [e2fsprogs](https://github.com/tytso/e2fsprogs) v1.47.4:
  Linux golden oracle, feature reference and potential future separately
  distributed/ported tools after license review.
- [lwext4](https://github.com/gkostka/lwext4):
  comparison only; as currently assembled it contains GPLv2 files.
- [WinFsp](https://github.com/winfsp/winfsp):
  only for later mount stage, governed by its GPLv3 special FLOSS exception.

## Benchmark engine selection

- Microsoft DiskSpd v2.3 (MIT) is selected as the first Windows benchmark engine candidate: https://github.com/microsoft/diskspd
- CrystalDiskMark provides familiar workload/UI references and internally invokes DiskSpd: https://github.com/hiyohiyo/CrystalDiskMark
- fio (GPLv2) is a differential reference, not a bundled engine: https://github.com/axboe/fio
- P2.5 specifications and release gates: [P2.5 benchmark plan](P2_5_DISK_BENCHMARK.md) and [ADR-0003](decisions/0003-diskspd-file-targets.md).
- P2.5A preview UI, disabled typed IPC, strict DiskSpd XML decoding, a pinned read-only CI DiskSpd integration, and P2.5B.1 workload/scratch lease primitives are implemented. P2.5B.2 adds a write-only DiskSpd rate ceiling and a conservative maximum-byte estimate; the estimate is not yet connected to user consent or a production worker. No production benchmark launch or disk I/O is enabled.

## Verified phase status (2026-10-19)

- **P0**: TypeScript/Rust skeleton, MIT license, threat model, locked dependency
  installs, CycloneDX Node SBOM, and Windows/Linux CI exist. Dependabot PR #13
  updated fast-xml-parser from 5.2.5 to 5.7.0; PR #16 default-denies Electron
  browser permissions and synchronizes docs; PR #18 pins CI actions, Node 24.21.0
  and Rust 1.99.1. All merged by 2026-10-18. Runtime/transitive dependency
  audits, production provenance and hardware validation are still outstanding.
- **P1**: Approved Windows 11-style mock UI and real-Electron CI screenshot
  tests are present. Packaged Windows 11 desktop testing, physical mixed-DPI,
  Narrator and high-contrast acceptance are NOT complete.
- **P2**: PR #49 added a bounded, cancellable metadata-only inventory endpoint
  to the Electron main process. It reports capacity, sector sizes, bus type
  and partition summary, but no serials, raw sectors or durable device IDs.
  PR #49 merged at `fdab284f936bf20a5bcecd5b28d4ca200280a4aa`; PR checks were
  5/5 and post-merge main checks were 3/3. Physical Windows 11 acceptance and
  stable hardware identity remain open.
- **P2.5A / P2.5B.1 / P2.5B.2 / P2.5B.3 / local P2.5B.4**: Four mock-only benchmark profiles, a strict XML result
  parser, genuine pinned DiskSpd **read-only CI** smoke test, fixed workload
  planner with a write-only `-g100000` cap and conservative byte estimate,
  exact-owned scratch-file tests, and protected MBR/GPT partition metadata
  carried by worker protocol v2 exist. PR #51 is merged and its post-merge
  `main` checks passed 3/3. The local P2.5B.4 slice adds bounded WMI
  drive-letter/partition mapping, system-disk exclusion, volume eligibility,
  and sanitized preview metadata; real Windows 11 volume acceptance is still
  pending. No user-facing benchmark process, owned scratch-file authorization,
  Job Object cancellation, consent screen or real-device performance reports
  exist.
- **P3–P8**: A local image-only, read-only ext reader candidate exists in `crates/ext-reader`, but is not wired into the app and has not passed independent differential/fuzz gates. Formatting, mount,
  repair, resize and raw disk writes remain unavailable.
- **P9**: GitHub Actions can package an unsigned, unpacked x64 UI preview;
  there is no signed end-user release or hardware acceptance.

## Release milestones

| Milestone | Contents | Blocking safety gate |
| --- | --- | --- |
| M1 Read-only Disk Viewer | P2 real disk discovery and P3 ext file browsing/export | Fuzzed and differential-tested parsers; no disk writes |
| M2 Disk Tools | M1 + P2.5 file-only benchmark + audited P4/P5 formatting | Disposable hardware and power/interruption tests |
| M3 Advanced Management | Conditional P6 repair, P7 resize, P8 mount | Per-operation conformance; no unverified rollback claims |

## Execution sequence and checkpoints

1. **P0-H**: Keep XML dependencies patched; audit runtime/transitive licenses;
   harden Electron's globally denied web permissions, preload and IPC;
   strengthen dependency/CI provenance; synchronize issue/ADR status.
2. **P1-QA**: Verify the packaged executable on actual Windows 11,
   keyboard/Narrator/forced-colors and mixed-DPI physical monitors.
3. **P2-I1..I6**: Image-only bounded reads; Win32 read-only enumeration;
   GPT/MBR differential fixtures; hotplug-safe opaque identities;
   versioned agent IPC; verified real-device UI (no writes).
4. **P2.5-B2..B6**: Pinned DiskSpd distribution, trusted non-system
   volume authorization, signed-off file ownership, bounded workloads,
   Windows Job Object tree cancellation and verified metrics/device QA.
5. **P3-I1..I5**: Continue validating the pinned `rust-fs-ext4` v0.8.0
   image-only evaluation wrapper; inspect feature flags, directories, streamed
   exports, unsafe targets, and corrupt/dirty images against Linux oracles.
6. **P4/P5**: Implement image-only writes first; verify with e2fsck and
   fault-injection. Require a separate, independently revalidating elevated
   agent and disposable-disk acceptance before any physical formatting.
7. **P6/P7/P8**: Conditional extensions only if an independently verified
   engine supports each operation. ext resize and partition resize are distinct.
8. **P9**: Sign installers, complete SBOM including Rust/native payloads,
   reproduce release toolchains, complete hardware regression and provenance.

Every increment: separate PR, tests on Windows/Linux where relevant,
explicit capability checks, docs/ADR updates, and no physical writes before P5.
Never equate passing GitHub-hosted VM tests with physical Windows 11 validation.

## P2-I2 current implementation split

- P2-I2a: native SetupAPI GUID_DEVINTERFACE_DISK discovery with an internal,
  bounded list of Windows interface paths. Diagnostic `discovery-count`
  prints the count only; all UI capabilities stay false. No device opens.
- P2-I2b: first bounded agent-only GENERIC_READ Win32 metadata diagnostic
  **merged** via [PR #24](https://github.com/illetyus/Partition-11/pull/24)
  (`3a917865`): temporary device number, capacity, logical/physical sector
  sizes and bus type, fail-closed elevation policy, Win32 errors and tests.
  **New P2-I2b native read-only layout query merged in PR #28
  (7cda1638):** 0x00070050 fixed GET IOCTL, bounded 128 partitions,
  data validated by the pure decoder, aggregate-only internal diagnostic.
  These are still transient, non-authenticated results; Windows SDK C ABI,
  hotplug and genuine Windows 11 hardware acceptance remain outstanding.
  No GUI disk access or write flags. The former status:
  This is NOT a stable identity or UI disk scan. A separate P2-I2b pure
  bounded Win32 drive-layout parser has been added (no live IOCTL yet);
  drive/partition layout querying,
  further native denial/hotplug tests and physical Windows 11 acceptance
  remain open under [#22](https://github.com/illetyus/Partition-11/issues/22).
  See [metadata design](P2_WINDOWS_DISK_METADATA.md).
- P2-I2c: bounded metadata I/O is merged; durable identity, physical hotplug
  acceptance and authenticated read-only agent IPC remain open in Issue #31.
- [P2-I2 parent #19](https://github.com/illetyus/Partition-11/issues/19).

## Previous checkpoint — P2-I2c-2 agent-private PnP instance cross-check

[PR #35](https://github.com/illetyus/Partition-11/pull/35) merged at
`b3d256ef` after 5/5 successful PR tests. The agent cross-checks
`SetupDiGetDeviceInstanceIdW` values with each observed disk-interface
path at the start/end of a non-elevated, read-only metadata scan. A
Windows SDK C ABI oracle validates `SP_DEVINFO_DATA`. Neither the path
nor the instance ID is a durable physical identifier. At this checkpoint,
rapid replug, PnP notification generation/cancellation, handle lifecycle,
and physical Windows 11 tests remained open. PR #39, #43 and #46 later added
event invalidation, per-HANDLE removal handling, and bounded I/O cancellation.
Durable identity, physical acceptance and authenticated read-only IPC/UI
remain open in [Issue #31](https://github.com/illetyus/Partition-11/issues/31).
No renderer disk access or write/elevation capabilities are enabled.

## Previous checkpoint — P2-I2c-4 Windows PnP device-interface event invalidation

[PR #39](https://github.com/illetyus/Partition-11/pull/39) merged at
`23c3e4f0`, following 5/5 PR checks. The diagnostic registers
`CM_Register_Notification` before the first SetupAPI scan and
invalidates its ephemeral atomic generation when Windows reports disk
interface changes. Unregistration waits for callbacks, then the scan
rechecks both the event generation and full device inventory. The
Microsoft SDK C ABI test and Windows VM subscribe/unsubscribe tests
passed. At that time, [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
still called for per-device handle notifications and blocked-I/O cancellation;
PR #43 and PR #46 later added those protections. Durable hardware identity,
physical Windows 11 acceptance and authenticated read-only IPC/UI remain open.
Do not turn on physical disk access, raw reads/writes, or UAC.

## Previous checkpoint — P2-I2c-1 bounded fail-closed scan inventory

[PR #33](https://github.com/illetyus/Partition-11/pull/33) merged at
`c0da73dc`: present disk interface inventory before/after the entire
metadata probe must remain unchanged, unique and within a 120-second
scan lifetime. Duplicate/unknown temporary disk numbers fail closed.
This is **not strong physical identity**: identical path reuse on rapid
remove/replug is not resolved. At this checkpoint, [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
also tracked PnP invalidation and I/O cancellation; PRs #39, #43 and #46
later implemented those parts. Durable identity and physical hardware
acceptance remain open. All Electron disk access and write capabilities
remain disabled.

## Previous checkpoint after P2-I2b Windows SDK ABI validation

- [PR #30](https://github.com/illetyus/Partition-11/pull/30) merged:
  independent MSVC + Windows SDK C11 compile-time ABI checks on Windows CI,
  including the exact parser offsets and GET-only disk-layout IOCTL;
  Windows fixture filename collisions were also fixed.
- At this historical checkpoint, [Issue #31](https://github.com/illetyus/Partition-11/issues/31)
  was next. PnP invalidation, per-HANDLE removal handling and bounded
  cancellable metadata I/O later landed in PRs #39, #43 and #46.
- Current remaining gates in Issue #31: durable hardware identity,
  physical Windows 11 USB/NVMe/SATA/BitLocker/512e/4Kn acceptance, and
  authenticated read-only IPC/UI. These remain **incomplete**.
- UI disk access, writing, formatting, mounting and elevation remain disabled.

## Continue from the latest handoff

**Session-independent checkpoint:** [SESSION_HANDOFF.md](SESSION_HANDOFF.md).
Read it before resuming in another ChatGPT/Codex session. It records the
last verified `main` SHA and CI evidence, merged changes, safety boundaries,
exact next P2-I2 issue, tests, and the **mandatory merge-to-main protocol**.
The checkpoint is a dated snapshot: verify live GitHub state each new session.

## Work tracking

- P0: [#1](https://github.com/illetyus/Partition-11/issues/1)
- P1: [#2](https://github.com/illetyus/Partition-11/issues/2)
- P2: [#3](https://github.com/illetyus/Partition-11/issues/3); current next task
  [P2-I2 #19](https://github.com/illetyus/Partition-11/issues/19)
- P2.5: [#9](https://github.com/illetyus/Partition-11/issues/9)
- Completed relevant PRs: [#6](https://github.com/illetyus/Partition-11/pull/6)
  memory parser, [#14](https://github.com/illetyus/Partition-11/pull/14)
  official read-only DiskSpd CI, [#15](https://github.com/illetyus/Partition-11/pull/15)
  scratch lease and workload planner.

- Latest completed P0/P2 changes: [#13](https://github.com/illetyus/Partition-11/pull/13) dependency, [#16](https://github.com/illetyus/Partition-11/pull/16) Electron security, [#17](https://github.com/illetyus/Partition-11/pull/17) internal read-only image reader, [#18](https://github.com/illetyus/Partition-11/pull/18) pinned CI. They are **merged**, not pending.
## 2026-10-20 — P2.5 main-process result validation checkpoint

The approved local P2.5 worktree now validates worker result envelopes and
reports in Electron main against the consumed one-use selection, exact
profile/direction, pinned DiskSpd XML version, fixed scratch/timing policy,
cache policy, timestamp and measurement bounds. It rejects extra fields and
projects the internal volume HMAC to the already visible one-use selection
token before sending a report to the renderer. Malformed output and uncertain
post-launch failures latch `cleanup_required`.

Fresh validation: `npm run test:main` 76/76, `npm run build` passed, and
`PARTITION11_UI_SMOKE_SKIP_CAPTURE=1 npm run test:ui:win` passed 42 mock-IPC
assertions. The ordinary capture path returned `UnknownVizError` on this local
compositor. `BENCHMARK_RUNTIME_ENABLED` is
still `false`; no worker dispatch, scratch file, DiskSpd process, physical
inventory, or volume write occurred. The Windows CI-only pinned DiskSpd
delete-on-close compatibility gate and all later P2.5 lifecycle, packaging,
and user hardware acceptance gates remain incomplete.

## 2026-10-20 — fresh local verification and remote gate status

The approved P2.5 worktree passed the safe local checks: full locked Rust workspace tests, formatting, warning-denying Clippy, release build, `npm run test:main` (76/76), `npm run build`, and capture-disabled Windows UI smoke (42 mock-IPC assertions). Two physical-agent probes, the process-tree helper, and the CI-only DiskSpd/delete-on-close probe remained ignored. Production npm audit reported zero vulnerabilities; full npm audit had eight Moderate dev-only `sprintf-js` records and no High/Critical findings. `cargo audit --file Cargo.lock` and the fuzz lockfile audit passed. The pinned DiskSpd executable SHA-256 matched its manifest and CycloneDX component; the target-specific agent SBOM includes `partition11-core`.

Live GitHub state was checked: `main` is `130868dbaf60a3dfa71f62f5d5086c985d792cc6` with 5/5 successful checks, but those checks do not cover this local worktree. No remote feature branch or open PR exists. The required Windows CI-only compatibility gate therefore remains unverified. Keep `BENCHMARK_RUNTIME_ENABLED` false and do not treat the old main checks as evidence for local changes. No physical inventory, DiskSpd launch, scratch creation, or volume write occurred.
## 2026-10-20 — P3 local fuzz extension

The existing ext-image reader fuzz target completed a 601-second Windows x64
run using the nightly-2026-10-17 toolchain and MSVC 14.50 selected for that
process only. It completed 10,091 inputs with exit code 0, 1,306 coverage
counters, 3,016 features, 138 new corpus units, and 1,034 MiB peak RSS; no crash
or timeout was reported. The two independent e2fsprogs fixtures were copied to
an isolated temporary corpus and their source hashes matched before and after.
A temporary command launcher was also consumed once as an arbitrary-byte seed
and then removed; repository fuzz seeds were unchanged. The retry did not
modify persistent environment settings. This does not verify the unrun hosted
CI fuzz job, abrupt-host-crash cleanup, or physical Windows 11 acceptance.
## 2026-10-20 — P3 actual agent fixture check

The freshly built Windows x64 `partition11-agent.exe` was invoked in
`image-worker` mode over its real bounded stdin/stdout protocol. Against the
independent e2fsprogs fixture it listed the five root entries, exported the
expected 36-byte `known.txt`, committed the output and left no stage file. An
existing-destination collision was rejected without changing the destination
or creating a stage. The source image hash was unchanged before/after, and all
output was removed from a unique temporary directory. This is native agent
fixture evidence only; it does not establish Electron UI end-to-end behavior
or physical Windows 11 acceptance.
## 2026-10-20 — P3 main-service integration and crash-cleanup research

The compiled Electron `ReadOnlyImageInspectionService` on Node 24.21.0 used its
default spawn path to the real Windows x64 agent. It listed the clean synthetic
image, exported the expected 36-byte file, rejected an existing destination,
left no stage after the successful run, and preserved the source hash. The
unique temporary output was removed. This does not claim rendered Electron IPC
or physical-device behavior.

Crash-recovery research found that Windows `FILE_FLAG_DELETE_ON_CLOSE` removes
a file after all handles close, but `FILE_DISPOSITION_INFO.DeleteFile` has no
effect when that flag was used. This rules out simply clearing delete-on-close
on success. No behavior was changed; the P3 recovery policy still needs its
separate design approval.