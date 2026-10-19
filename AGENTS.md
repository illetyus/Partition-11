# Partition 11 — Agent / future-session instructions

## LATEST MERGED CHECKPOINT — P2.5B.3 partition protection metadata (2026-10-19)

- PR #51 merged as `eb2d932abdb492a040232fbbc3bc81ae558ce467` after 5/5 PR
  checks. Fresh `main` at that SHA passed CI, Windows UI, and DiskSpd XML
  checks 3/3:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39309441359),
  [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39309441521),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39309441451).
- The parser and metadata worker protocol v2 retain MBR active/type and GPT
  partition type information for later benchmark protection policy. PR #50's
  bounded write rate and conservative byte estimate remain internal planning
  primitives; no benchmark process or volume write is enabled.
- Next implementation branch: `feature/p2.5-volume-eligibility-policy` from
  the verified `main`. Implement trusted volume-to-partition mapping and
  fail-closed system/boot/recovery, filesystem, identity, mount, and free-space
  eligibility before scratch file creation, write consent, or a worker.
- Never enable a live benchmark until all P2.5 gates pass. Physical Windows
  11 device acceptance remains the user's test gate; do not simulate it with a
  hosted VM result.
- PR #49 added the bounded metadata-only physical disk inventory endpoint;
  it does not expose raw sectors, serials, or durable device IDs. Physical
  Windows 11 USB/NVMe/SATA/BitLocker/sector-size acceptance remains the user's
  test gate in Issue #31.
- The user's canonical repo is `D:\Partition 11`; shared general build
  dependencies belong under `D:\Android`. The user performs physical Windows
  11 device acceptance; never substitute a hosted VM result for that gate.

---

## LATEST MERGED DEVELOPMENT — bounded/cancellable metadata I/O (2026-10-19)

- [PR #46](https://github.com/illetyus/Partition-11/pull/46) merged to
  `main` as `a9cb53344ff560082096d728eb453af0532a7415` after **5/5 PR checks**.
- Roadmap item **2 is complete at code/CI level**: metadata IOCTLs run in a
  disposable worker using overlapped I/O, a 5-second request budget that
  starts before submission, exact-request `CancelIoEx`, and a 1-second
  completion grace. The worker and parent enforce independent 20-second
  limits; timed-out, invalidated, or malformed output is discarded, and
  stuck child/pipe resources are quarantined before another worker starts.
- Fresh post-merge `main` checks are **5/5 successful** at the same SHA:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39195989162),
  [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39195989210),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39195989174).
- Details and boundaries: [docs/P2_I2C_BOUNDED_IO.md](docs/P2_I2C_BOUNDED_IO.md).
  No Electron disk capability, raw-sector access, write, format, mount,
  repair, resize, or elevation is enabled.
- [Issue #31](https://github.com/illetyus/Partition-11/issues/31) remains
  open for durable hardware identity, physical Windows 11 USB/NVMe/SATA/
  BitLocker/512e/4Kn acceptance, authenticated read-only IPC and UI work.

---

## PREVIOUS MERGED DEVELOPMENT — per-HANDLE removal lifecycle (2026-10-19)

- [PR #43](https://github.com/illetyus/Partition-11/pull/43) merged to `main` as `b17e0fbeacd9ee28f7383b5a75a1d7a8198f6a43` after **5/5 successful PR checks**.
- Requested roadmap item **1 is complete at code/CI level**: every successfully opened metadata disk HANDLE gets a `CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE` registration. Idle query-remove closes the exact HANDLE in the callback; a HANDLE currently leased to synchronous `DeviceIoControl` is never closed underneath that call. Query-remove is vetoed with `ERROR_CANCELLED`, the scan is invalidated, and the HANDLE is closed immediately when the call returns.
- Registration races are fail-closed: the filter HANDLE remains valid until `CM_Register_Notification` returns. `DEVICEREMOVEPENDING` / `DEVICEREMOVECOMPLETE` also invalidate and close/defer-close. `DEVICEQUERYREMOVEFAILED` never resurrects a stale scan; the next invocation re-enumerates and opens a fresh HANDLE.
- Windows SDK C oracle independently verifies the HANDLE-union offset, filter/action enum values and `ERROR_CANCELLED`. Scope/design: [docs/P2_I2C_HANDLE_REMOVAL.md](docs/P2_I2C_HANDLE_REMOVAL.md).
- Fresh post-merge `main` verification is **5/5 successful**: [CI](https://github.com/illetyus/Partition-11/actions/runs/39179456003), [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39179456177), [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39179455971).
- At the time PR #43 merged, roadmap item 2 was still pending. It is now
  implemented by [PR #46](https://github.com/illetyus/Partition-11/pull/46)
  and verified on fresh `main` above. Durable hardware identity and physical
  Win11 hotplug acceptance remain later gates.

---


## PREVIOUS MERGED DEVELOPMENT — P2-I2c metadata query epoch (2026-10-19)

- [PR #41](https://github.com/illetyus/Partition-11/pull/41) merged as `6b9f293dea427f598fdc35ed355d9d39f5d6d3dd`, after 5/5 successful PR checks. Tests were written first and failed as expected (missing `ProbeEpoch`), then Windows/Linux Rust and Electron/Windows UI/DiskSpd passed.
- The Rust agent now checks a single PnP `ScanGeneration` epoch **before and after every synchronous metadata query** and each partition-layout retry. On cancellation it drops results and starts no further Win32 metadata requests. It still **cannot forcibly terminate a blocking synchronous IOCTL**.
- Scope and Microsoft device removal semantics: [docs/P2_I2C_QUERY_EPOCH.md](docs/P2_I2C_QUERY_EPOCH.md). No per-device HANDLE query-remove callback, durable hardware identity, authenticated agent IPC, physical Win11 tests or GUI disk access is implemented.
- [Issue #31](https://github.com/illetyus/Partition-11/issues/31) remains OPEN: next design/review handle-specific removal notifications with safe handle+in-flight I/O lifetime; assess hardware-backed identifiers and test physical USB/NVMe/SATA/512e/4Kn/BitLocker.
- Post-merge `main` CI: [CI](https://github.com/illetyus/Partition-11/actions/runs/39176273055), [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39176273089), [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39176273046). **Verify final conclusions before claiming post-merge success.** All inspection and write capability flags remain false.

---


## PREVIOUS VERIFIED CODE MERGE — Windows PnP scan invalidation (2026-10-18)

- [PR #39](https://github.com/illetyus/Partition-11/pull/39) **merged into
  `main`** at `23c3e4f03ede81c1ba7cc94bffa69aa298065bd6`
  after **5/5 successful pre-merge checks**. One DiskSpd run failed from
  an external Microsoft ZIP network timeout and was **rerun successfully**
  on the same commit; checks were not bypassed.
- New `pnp_notifications.rs`: a non-elevated short-lived
  `CM_Register_Notification(GUID_DEVINTERFACE_DISK)` subscription starts
  before first SetupAPI enumeration, sending arrival/removal events to
  the same agent-private `ScanGeneration`. Callback does atomic
  invalidation only; it never opens a disk or blocks.
- After metadata queries, the observer unregisters and checks the generation
  and full PnP inventory again. Missing notification registration or
  teardown failure returns an error; no renderer or user-visible disk
  access is enabled. The Windows SDK C ABI oracle validates the x64
  `CM_NOTIFY_FILTER`; hosted Windows registration and synthetic callback
  tests passed. **This is NOT real hardware hotplug acceptance.**
- Fresh `main` tests for this merge:
  [CI](https://github.com/illetyus/Partition-11/actions/runs/39164828649),
  [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39164828633),
  [DiskSpd XML](https://github.com/illetyus/Partition-11/actions/runs/39164828608).
  **Verify the final outcomes before calling post-merge green.**
- **Next issue remains [P2-I2c #31](https://github.com/illetyus/Partition-11/issues/31):**
  implement handle-specific query-removal notifications / safe handle
  lifecycle, robust hardware-backed identity where available, cancellation
  during blocking IOCTL, actual Windows 11 USB/NVMe/SATA/BitLocker testing,
  authenticated IPC design. All four Electron physical/image inspection
  and write flags remain false, no raw sector reads or writes, no format/UAC.


## PREVIOUS CHECKPOINT — P2-I2c-3 scan-generation primitive MERGED

[PR #37](https://github.com/illetyus/Partition-11/pull/37) merged into
`main` as `fd920434101ce5b7797caced9d0c451caf86dc2e` after
**5/5 green PR checks**, resolving the Windows Clippy dead-code findings.
The agent now has a private atomic `ScanGeneration` to invalidate stale
scan snapshots; normal aggregate-only metadata scans explicitly invalidate
their private generation once a scan completes. Tests cover cancelled old
snapshots, multiple observers and guard isolation. Only a test-only
convenience constructor is excluded from production lint scope.

**Historical note:** this checkpoint described PR #37 before later work.
PR #39 added Windows PnP event invalidation, PR #43 added per-HANDLE
query-removal lifecycle handling, and PR #46 added bounded cancellable
metadata I/O in a disposable worker. This section no longer defines the
current next step. Remaining gates are durable hardware identity, physical
Windows 11 device acceptance, and authenticated read-only IPC/UI; see the
latest checkpoint above and [Issue #31](https://github.com/illetyus/Partition-11/issues/31).

Post-merge `main` CI for that SHA (CHECK FINAL STATUS before claiming success):
[CI](https://github.com/illetyus/Partition-11/actions/runs/39144208924),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39144208918),
[DiskSpd XML](https://github.com/illetyus/Partition-11/actions/runs/39144208949).

At this historical checkpoint, the next work was to connect Windows PnP
notifications to a shared agent generation; that work landed in PR #39.
Per-HANDLE removal handling and bounded in-flight I/O cancellation later
landed in PRs #43 and #46. Current remaining work is durable hardware
identity, physical Windows 11 acceptance, and authenticated read-only IPC/UI.
Preserve the disabled disk capability flags until those gates are accepted.

---


## PREVIOUS MERGE — P2-I2c-2 PnP instance ID pair validation (2026-10-18)

**[PR #35](https://github.com/illetyus/Partition-11/pull/35) MERGED to main**
at `b3d256ef92ebe966fb1463304195f0874725fa21` with **5/5 green PR
checks**. The Windows agent now pairs each SetupAPI GUID_DEVINTERFACE_DISK
locator with its `SetupDiGetDeviceInstanceIdW` value from the same live
device-information set and checks the complete (path, instance ID) mapping
before/after the internal metadata probe. Duplicate/invalid IDs and changed
mapping reject the whole scan; a standalone Windows SDK C ABI check covers
`SP_DEVINFO_DATA`. No paths or IDs are exported to the renderer.

**Critical limitation:** PnP instance IDs are OS-reported devnode identifiers,
NOT hardware-backed, immutable identities. Rapid remove/replug reusing the
same instance ID/path remains undetected. #31 is NOT complete. Next work:
[P2-I2c Issue #31](https://github.com/illetyus/Partition-11/issues/31)
PnP event notification and agent-owned generation/expiry/cancellation;
evaluate trustworthy hardware identity only where available, then physical
Windows 11 hotplug / device-matrix acceptance. Do not enable Electron disk
access or disk writes, formatting, elevation, mounting or benchmark.

Post-merge `main` CI started (verify final result before claiming 5/5):
[CI](https://github.com/illetyus/Partition-11/actions/runs/39138840885),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39138840928),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39138840905).
Follow tested PR → main merge → fresh main CI → issue/handoff update.


## PREVIOUS MERGE — P2-I2c-1 fail-closed scan inventory (2026-10-18)

[PR #33](https://github.com/illetyus/Partition-11/pull/33) **merged into main**
at `c0da73dc94907e004907d52409d011805c7f765e` after five passing PR
checks. It adds `crates/windows-agent/src/scan_identity.rs` and enforces
**full-scan rejection** if paths appear, disappear, duplicate, get
reassigned, successful disk-number probes collide, or the scan exceeds
120 seconds. Local diagnostic output is aggregate-only.

The observed inventory is **not a durable device identity**: a very fast
unplug/replug may reuse the *same path and OS disk number* undetected.
**Next work remains P2-I2c [Issue #31](https://github.com/illetyus/Partition-11/issues/31)**:
independent Windows PnP instance identity and reliable storage identifier
evaluation, ephemeral authenticated per-handle generation and invalidation,
cancel/removal tests and real Windows 11 hotplug acceptance. Do not enable
Electron disk access, raw sector reads, writes, formatter, mount or UAC.

Post-merge main checks:
[CI](https://github.com/illetyus/Partition-11/actions/runs/39133308019),
[Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39133308056),
[DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39133308116).
**Verify the final outcomes in a new session; this is dated evidence.**
Follow required: feature PR, 5/5 green, main merge, new main CI, update issue
and handoff.


## CURRENT VERIFIED MERGE — Win32 SDK ABI oracle, PR #30 (2026-10-18)

- [PR #30](https://github.com/illetyus/Partition-11/pull/30) merged to
  `main` as `df74e420827afd8815dba764b4814664ec7fbf1c` after
  **five passing pre-merge checks**.
- The Windows CI now compiles and runs a standalone Microsoft C11/WinSDK
  oracle, checking the exact field offsets/strides and fixed GET IOCTL
  that the Rust partition-layout decoder actually uses.
- Important SDK difference: Windows Kits `10.0.26100.0` omits the
  `PartitionOrdinal` member described by one online reference and has
  a different MBR union-arm size. **Neither is used by the decoder**;
  all consumed ABI offsets and sizes passed the independent compiler test.
- Fixed flaky test fixture filenames in `crates/core/src/image.rs`
  using an atomic sequence and collision-safe `create_new` retries.
- New `main` CI runs: [CI](https://github.com/illetyus/Partition-11/actions/runs/39129897257),
  [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39129897239),
  [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39129897256).
  **Check their current final conclusions before claiming post-merge green.**
- **Exact next development issue:** [P2-I2c #31](https://github.com/illetyus/Partition-11/issues/31)
  for agent-owned, short-lived disk identity snapshots and surprise-removal/
  replug invalidation. The previous [P2-I2b #22](https://github.com/illetyus/Partition-11/issues/22)
  documents remaining hardware/ARM64/MBR cases.
- No physical Windows 11 hardware acceptance, raw sector exposure,
  authenticated renderer IPC, UAC, formatting, write, repair or resize.

Always check the live GitHub `main` and open issues before resuming;
complete work only after required checks, `main` merge, fresh `main`
CI, and updating this handoff.



## CURRENT CHECKPOINT — P2-I2b Win32 layout read MERGED

[PR #28](https://github.com/illetyus/Partition-11/pull/28) merged as
`7cda163816a5994cf894ff3ff48c488df77fb5a4` after 5/5 green PR checks.
The non-elevated Rust agent has a bounded fixed `IOCTL_DISK_GET_DRIVE_LAYOUT_EX`
GET-only metadata query feeding the pure `windows_layout` decoder. Local
diagnostics report **aggregate counts only**. This is not a complete disk
viewer; there is **no Electron IPC, raw sector I/O, UAC, write, format,
repair, resize or real benchmark**.

**Next:** [P2-I2b/#22](https://github.com/illetyus/Partition-11/issues/22)
Windows SDK C ABI oracle and independent Win32 error/identity/hotplug
validation; parent [#19](https://github.com/illetyus/Partition-11/issues/19).
Follow the mandatory green checks → merge-to-`main` → post-merge CI →
handoff rule below. Verify latest live `main` and not just this dated SHA.


## PREVIOUS VERIFIED MERGE (2026-10-18)

[PR #26](https://github.com/illetyus/Partition-11/pull/26) merged to `main`
as `e74a9d7576b624ac05c05948055a268b3104c6ec` (5/5 PR checks passed).
`crates/core/src/windows_layout.rs` is a pure, bounded Win32 drive-layout
response decoder; **it does not call DeviceIoControl, access disks or expose
anything to Electron**. The prior non-elevated agent metadata diagnostic from
PR #24 remains separate. The next task is [P2-I2b #22](https://github.com/illetyus/Partition-11/issues/22):
integrate a fixed **read-only** drive-layout IOCTL with strict Windows ABI
and returned-length validation, then hotplug/identity acceptance. Physical
writes, format, UAC and UI disk access remain forbidden. Check live `main`
and its latest CI, then follow the mandatory PR → 5 green checks →
`main` merge → post-merge CI → handoff update rule below.


## CURRENT RESUME POINTER — 2026-10-18 (supersedes older snapshot below)

**PR #24 P2-I2b first slice merged** to main as `3a917865f821f422dffffd04dbdf0549ef0e6ce6` (2026-10-18). Read-only Windows agent metadata diagnostic now opens non-elevated GENERIC_READ handles for hardcoded metadata IOCTLs and returns aggregate-only counts, not sectors or renderer-visible paths. Pre-merge CI, Windows UI and DiskSpd passed. See [docs/SESSION_HANDOFF.md](docs/SESSION_HANDOFF.md).
**Next issue remains [P2-I2b #22](https://github.com/illetyus/Partition-11/issues/22)** (bounded drive layout/partition metadata, stronger identity, read-only/hotplug/hardware acceptance); parent [P2-I2 #19](https://github.com/illetyus/Partition-11/issues/19) open. **No raw reads/writes, UI disk access, UAC, formatter or user benchmark.** Recheck new `main` and push CI. Merge every checked PR and verify post-merge `main` CI. Historical resume bullets below remain for provenance.

**Read this file first.** The authoritative human-readable, dated checkpoint
is [docs/SESSION_HANDOFF.md](docs/SESSION_HANDOFF.md). Read it **in full**,
then [docs/PLAN.md](docs/PLAN.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
[SECURITY.md](SECURITY.md), and open GitHub Issue
[P2-I2 #19](https://github.com/illetyus/Partition-11/issues/19).

## Operating rules

1. Start every session by querying live GitHub `main` HEAD, open PRs/issues
   and Actions statuses. A dated handoff SHA is a historical snapshot, not
   proof that the repository has not changed.
2. Work in small scoped branches with a PR, tests, and documentation. Keep
   the user's original architecture and implementation; don't rewrite it.
3. **Mandatory merge to `main`**: after all required checks pass, squash
   merge the reviewed PR. Verify the new `main` SHA and post-merge CI
   before reporting completion. Never treat "PR open" as "merged".
4. After each increment, update the GitHub issue checkboxes, plan and
   [docs/SESSION_HANDOFF.md](docs/SESSION_HANDOFF.md), including the next
   issue, actual merge SHA, and post-merge CI evidence.
5. Safety: Electron renderer is untrusted. Real disk and image capabilities
   remain disabled until a separately authenticated Rust agent, proper
   identity/TOCTOU checks and read-only acceptance tests exist. **Never
   enable disk writes, formatting, repair, resize, elevation or raw
   benchmarking early**. Keep mock UI explicitly labeled.
6. Explain progress concisely to the user in **Turkish**, avoiding unexplained
   technical jargon. Say when something is untested or pending.

## Resume here (snapshot 2026-10-18)

- Last completed development PRs: #13, #16, #17 and #18 all merged.
- Verified baseline main SHA: `2b4a7cd0ecfeb2b246c4f917a500c96e41f6bf3c`;
  main CI, Windows Electron UI preview and read-only DiskSpd CI all green.
- Current next issue: [#19 — P2-I2 Native Windows read-only disk discovery](https://github.com/illetyus/Partition-11/issues/19).
- P2-I1 `crates/core/src/image.rs` is an **internal** read-only regular-image
  primitive only. It is not wired to the agent or renderer. All runtime
  disk/image/write capability flags still report **false**.
- Current target product: Windows 11 EXT2/3/4 manager without user-facing
  Linux/WSL dependency; release milestones M1 read-only viewer, M2 vetted
  benchmark/formatting, M3 conditional advanced operations.

See full evidence and exact source locations in the handoff. Always re-check
live repo state before changing code.
