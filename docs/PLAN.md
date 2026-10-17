# Partition 11 implementation plan

Last reviewed: 2026-10-18. Application name is **Partition 11** and the
GitHub repository remains **illetyus/Partition-11** by user decision.

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
- P2.5A preview UI, disabled typed IPC, strict DiskSpd XML decoding, a pinned read-only CI DiskSpd integration, and P2.5B.1 workload/scratch lease primitives are implemented. No production benchmark launch or disk I/O is enabled.

## Verified phase status (2026-10-18)

- **P0**: TypeScript/Rust skeleton, MIT license, threat model, locked dependency
  installs, CycloneDX Node SBOM, and Windows/Linux CI exist. Dependabot PR #13
  updated fast-xml-parser from 5.2.5 to 5.7.0; PR #16 default-denies Electron
  browser permissions and synchronizes docs; PR #18 pins CI actions, Node 24.21.0
  and Rust 1.99.1. All merged by 2026-10-18. Runtime/transitive dependency
  audits, production provenance and hardware validation are still outstanding.
- **P1**: Approved Windows 11-style mock UI and real-Electron CI screenshot
  tests are present. Packaged Windows 11 desktop testing, physical mixed-DPI,
  Narrator and high-contrast acceptance are NOT complete.
- **P2**: A bounded, memory-only GPT/MBR parser and **internal**, ordinary-file,
  bounded, read-only image reader (PR #17) exist. Neither is wired to the app
  or agent. Real Win32 disk discovery, stable device identity, end-to-end
  authenticated image access, ext superblock inspection, and physical-device
  acceptance are NOT implemented.
- **P2.5A / P2.5B.1**: Four mock-only benchmark profiles, a strict XML result
  parser, genuine pinned DiskSpd **read-only CI** smoke test, fixed workload
  planner, and exact-owned scratch-file tests exist. No user-facing benchmark
  process, production volume authorization, Job Object cancellation or
  real-device performance reports exist.
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
