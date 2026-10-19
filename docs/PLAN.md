# Partition 11 implementation plan

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

---

## Current local checkpoint — P2.5B.4 volume eligibility preflight (2026-10-19)

Work is in `feature/p2.5-volume-eligibility-policy` from the current verified
`origin/main`. This local slice adds read-only drive-letter-to-partition
mapping through the documented `Win32_LogicalDiskToPartition` association,
then matches the result against the agent's independent disk layout. It does
not open a volume device handle: Microsoft documents that direct volume
handles require administrator privileges, which the agent rejects.

The bounded local WMI query is run through a fixed Windows PowerShell command,
with a 3-second CIM operation timeout, a 5-second process deadline and a
512-KiB/4,096-row output limit. A second mapping snapshot and the existing
PnP/layout checks reject a changed scan. Missing or inaccessible WMI mapping
leaves all candidates ineligible. The UI receives only the sanitized drive
letter, metadata and an Electron-minted scan-local selection token; volume
GUID paths remain inside the agent. Eligibility denies the Windows system
disk, boot/protected or unknown partitions, ambiguous drive/folder mounts,
unsupported filesystem/drive types, read-only volumes and less than 2 GiB
free space. The 2-GiB floor allows the proposed 1-GiB scratch file plus a
1-GiB reserve.

Validation: 94 Rust agent tests passed (the live inventory probe was skipped), workspace Clippy and Windows x64 release agent build passed, TypeScript checks passed, main tests passed 42/42, production Electron build passed, and UI smoke passed with 45 assertions and 11 screenshots. The smoke used static mock data and verified launch does not query physical disks.

A fresh unsigned, unpacked x64 test candidate was packaged from source commit 3ee9d47 (documentation checkpoint f24fabc) on Windows 11 IoT Enterprise LTSC build 26100 (64-bit), using the CI-pinned Node 24.21.0 / npm 11.19.0 toolchain and Rust 1.99.1: release/Partition-11-Windows-x64-p2.5b4-f24fabc-node24.21.zip (207,806,797 bytes). SHA-256: C08E98102F1F78AB532F41F588E2376FBF2F49CCCA0E81E7695243990D401C79. ZIP CRC test passed for all 73 entries. App EXE SHA-256: 8D648AEEB305CC6D7876781B52BB02224F863F47E72D443FFC95431A6416580E; agent SHA-256: D2C434D941B7340A14A79958ED8D58F221436B718A43A1F63904CCA37CEDC9F7; app.asar SHA-256: C0EB45D86C62D6DB2C831109073EA6A55EA9796A9A38A892BB68C7BCEE46B2A1. The packaged agent capability probe reports image/physical inspection and all write capabilities false. This is an unsigned directory-preview candidate; signing, installer packaging, full release reproducibility, and physical Windows 11 acceptance remain open.

No live inventory or physical-device acceptance was run. The user will perform physical Windows 11 acceptance. Benchmark launch and disk writes remain disabled. No PR or merge has been made for this local checkpoint.


Primary API references: [volume direct-access requirements](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew),
[logical disk to partition association](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-logicaldisktopartition),
[partition disk index and range](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-diskpartition),
[local CIM association query](https://learn.microsoft.com/en-us/powershell/module/cimcmdlets/get-cimassociatedinstance?view=powershell-5.1).

Next: prepare the reviewed branch as a PR, complete required CI, and request the user's merge approval after the checks pass. Remaining P2.5 gates include owned scratch-file creation and identity, pinned DiskSpd execution, consent, Job Object process-tree cancellation, metrics validation, and the user's physical Windows 11 test.

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
- **P3–P8**: No ext read/write engine is integrated; formatting, mount,
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
5. **P3-I1..I5**: Select and pin an ext reader after feature/license review;
   inspect ext feature flags, directories, stream file exports, reject unsafe
   targets and test corrupt/dirty images against Linux reference oracles.
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
