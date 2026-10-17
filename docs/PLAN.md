# Partition 11 implementation plan

Last reviewed: 2026-10-17. Application name is **Partition 11** and the
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
| P3 ext read-only | File listing and export for supported ext feature flags, no journal replay writes | Linux/e2fsprogs differential tests |
| P4 Image-only writes | mkfs, metadata edits, fault-injection tests, safe error reporting | Images only; e2fsck -fn; power-failure tests |
| P5 Physical-device formatting | Dedicated privileged agent, strict lock, UAC, explicit plans and hardware tests | Disposable hardware acceptance |
| P6 Repair | Separate fsck audit and limited repair; never claim full e2fsck equivalence | Per-repair-class verified oracle |
| P7 Resize | Distinguish GPT/MBR partition resizing from ext filesystem resizing | Both directions with failure rollback strategy |
| P8 Optional mount | WinFsp licensing and I/O validation; start read-only | Legal/security review + conformance |
| P9 Release | Signing, packaged installers, SBOM, privacy, updater policy | Reproducible CI, release provenance |

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

## Phase status

P0 foundation is implemented, baseline GitHub CI passed on Windows and Linux.
Dependency locks are generated on the separate P0 branch for review.
P1 source design follows the final approved reference, including a mock disk
sidebar, format form, advanced settings and native Windows caption overlay.
A real Electron UI smoke workflow captures sample-device Windows screenshots
and compiles an unpacked preview; mixed-monitor DPI and a signed installer
remain unverified.
All physical-device and image operations remain disabled.

## Next actions

1. Merge P0 dependency lockfiles and convert CI to `npm ci` and Cargo `--locked`.
2. Archive a dependency SBOM as a CI artifact and audit release licenses.
3. Validate packaged Electron preload and Windows 11 mixed-DPI transitions.
4. Complete the P1 Narrator and contrast acceptance checklist.
5. Design and implement P2 image-only GPT/MBR parsers, followed by strictly
   read-only Win32 discovery behind narrow versioned IPC.
