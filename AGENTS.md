# Partition 11 — Agent / future-session instructions

## CURRENT RESUME POINTER — 2026-10-18 (supersedes older snapshot below)

**PR #21 P2-I2a merged** as `defe17f610aaa770cc8ffa18a4b9d9fea6c975ce` after all five PR checks passed. Full details: [docs/SESSION_HANDOFF.md](docs/SESSION_HANDOFF.md).
**Next issue: [P2-I2b #22](https://github.com/illetyus/Partition-11/issues/22)**; parent [P2-I2 #19](https://github.com/illetyus/Partition-11/issues/19) remains open. Add read-only Windows metadata/sector geometry to the agent, not real disk access via UI. **No raw writes, UAC, formatter or benchmark run enabled.** Recheck live `main` and post-merge CI before changes. Every checked PR MUST merge to main, then verify main CI and refresh this checkpoint. Historical resume bullets below remain for provenance only.

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
