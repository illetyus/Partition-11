# Partition 11 — Agent / future-session instructions

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


## LATEST VERIFIED MERGE (2026-10-18) — READ FIRST

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
