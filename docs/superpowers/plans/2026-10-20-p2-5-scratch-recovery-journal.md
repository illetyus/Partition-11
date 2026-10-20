# P2.5 scratch recovery journal — implementation plan proposal

> Status: Draft; awaiting explicit user approval of the specification and this plan.
> Worktree: `D:\Partition 11\.worktrees\readonly-disk-preview`
> Prerequisite: [scratch-recovery specification](../specs/2026-10-20-p2-5-scratch-recovery-journal.md) and [ADR-0004](../../decisions/0004-p2-5-scratch-recovery-journal.md).
> Until approved: do not replace ScratchLease, change cleanup semantics, enable benchmark IPC/UI, or run physical-device tests.

## Goal

Make the pinned official DiskSpd v2.3 target open compatible with an application-owned scratch file while preserving exact-object cleanup and fail-closed crash recovery. Keep `BENCHMARK_RUNTIME_ENABLED = false` until all automated gates pass. The user remains responsible for physical Windows 11 acceptance.

## Task 1 — Pin the incompatibility regression

Files: `crates/benchmark-worker/src/scratch.rs`, `.github/workflows/windows-diskspd-xml.yml`.

1. Keep a red test that creates a disposable delete-pending file and asserts the pinned DiskSpd-compatible open fails with `ERROR_SHARING_VIOLATION`; never turn this into an expected-success delete-on-close test.
2. Add a normal-file share-matrix test proving a non-delete-pending owner handle without DELETE access permits the exact pinned DiskSpd open contract.
3. Confirm the tests run only on temporary files and never target or enumerate host disks.

## Task 2 — Replace ScratchLease only after approval

Files: `crates/benchmark-worker/src/scratch.rs`, worker tests.

1. Add failing tests for no DELETE access and no delete-on-close while the lease is active, exact identity retention, fixed size, reparse/replacement rejection, and no inherited file handle.
2. Implement CREATE_NEW with a restrictive current-user/SYSTEM DACL and a share matrix that permits the pinned engine's read/write open.
3. Keep volume-GUID root validation, 128-bit file ID, volume serial, one-link and reparse checks, and bounded aligned size.
4. Implement exact-handle cleanup only after the benchmark job is verified empty: close the owner handle, reopen with `DELETE | FILE_READ_ATTRIBUTES` and share mode zero, revalidate the captured object, set disposition, close the delete handle, and verify absence. Any sharing conflict fails closed.

## Task 3 — Add protected, atomic lease journal

Files: new journal module under `crates/benchmark-worker/src/`, worker tests, and a narrowly scoped Electron/agent integration boundary.

1. Define and strictly parse a versioned record with lease ID, exact volume/root and file identities, path, size, timestamp, and lifecycle state.
2. Create the per-user journal directory with current-user/SYSTEM-only ACL. Reject unexpected owner/ACL, symlink/reparse records, oversized data, duplicate IDs, unknown fields/states, and malformed identities.
3. Persist each transition by write/flush/atomic replace in the same directory before allowing the next side effect. Cover interrupted writes and idempotent recovery.
4. Treat the create-to-file-ID-commit window as an unresolved state: never delete the path without a captured file ID; latch cleanup-required and block new runs.

## Task 4 — Integrate cancellation, shutdown, and recovery

Files: `crates/windows-agent/src/benchmark_run.rs`, `src/main/main.ts`, and related tests.

1. Persist `running` before spawning the pinned process. The renderer cannot choose a journal path or request cleanup.
2. On cancellation or failure, drain the Job Object before cleanup. `Undrained` retains the lease and journal and blocks another run.
3. On startup and when a volume returns, inspect only protected journal records. Reopen the exact path, compare volume identity/file ID/size and metadata, and delete only the validated file handle.
4. Missing, corrupt, mismatched, inaccessible, or unresolved records keep `cleanup_required`; never scan a volume or delete by prefix.
5. Return only sanitized cleanup state through main-process-owned UI state. Do not return internal file paths or disk identifiers to the renderer.

## Task 5 — CI, package, and release gates

Files: `.github/workflows/windows-diskspd-xml.yml`, Windows UI workflow, package checks, and relevant docs.

1. Run the exact pinned DiskSpd hash with a fixed read-only workload on a disposable runner-temp file using the ordinary shareable lease.
2. Assert XML target/workload/version, bounded process exit, job drain, exact file-ID-bound deletion, and absence after cleanup.
3. Exercise recovery fixtures and every no-delete case in Windows CI. Keep all host disk enumeration and physical media out of CI.
4. Re-run locked Windows/Linux workspace tests, rustfmt, warnings-as-errors Clippy, Electron main tests/build/mock UI smoke, dependency audits, SBOM, and unpacked package checks.
5. Keep preview unsigned and benchmark runtime disabled until all P2.5 gates pass. Do not merge before required reviews and CI; do not perform the user's physical Windows 11 acceptance.

## Acceptance criteria

- Pinned DiskSpd opens only the exact owned scratch file under the tested sharing matrix.
- No delete access or delete-pending state exists while DiskSpd is running.
- Cleanup occurs only after verified Job Object drain and only for the journal-bound file identity.
- Crash recovery is idempotent, exact-object-only, and fail-closed for incomplete or mismatched evidence.
- No cleanup path reports success while the file remains or the journal is unresolved.
- Benchmark capability stays disabled until all implementation and automated release gates pass; no Codex physical-device or volume benchmark is run.
