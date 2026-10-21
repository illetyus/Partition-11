# P2.5 scratch lease crash-recovery design

Status: **Approved for implementation by the user on 2026-10-21.** Approval does not authorize runtime enablement or physical-device testing.
Date: 2026-10-20; approval recorded 2026-10-21
Supersedes: none. It proposes an amendment to Task 1 of `2026-10-20-p2-5-production-benchmark-integration.md`.
Decision record: [ADR-0004](../../decisions/0004-p2-5-scratch-recovery-journal.md).

## Problem and evidence

Pinned DiskSpd v2.3 opens its target with `FILE_SHARE_READ | FILE_SHARE_WRITE` and does not include `FILE_SHARE_DELETE` ([upstream source at the pinned v2.3 commit](https://github.com/microsoft/DiskSpd/blob/5e7025bfc9d1364f185d4c30963d7ac79195435e/IORequestGenerator/IORequestGenerator.cpp#L1693-L1698)). Windows checks each new handle's desired access against the share mode of existing handles. A delete-on-close lease is incompatible with DiskSpd's open contract, and the current ScratchLease's in-flight DELETE access conflicts with the same fixed sharing mode.

Evidence on the approved P2.5 branch:

- GitHub DiskSpd run `39458367882`, head `cf2f3da7196b5251fd25ce1d4f1028274fe2d103`, failed with `exit_code=1` and stderr `Error opening file: <probe-file> [32]`.
- A local Win32 disposable-file probe, with no DiskSpd executable, reproduced error 32 for delete-on-close followed by a DiskSpd-shaped open even when the first handle omitted explicit DELETE access.
- The proposed share matrix was tested locally: an ordinary owner handle with read/write access and `FILE_SHARE_READ | FILE_SHARE_WRITE` (no DELETE access or share) allowed the DiskSpd-shaped open. DELETE opens returned error 32 while either handle remained open. After the DiskSpd-shaped handle and then the owner handle closed, an exclusive DELETE handle set `FILE_DISPOSITION_INFO` and the file disappeared.
- No physical disk inventory, physical-volume benchmark, or local DiskSpd workload was run.

## Proposed behavior

1. Keep the exact official DiskSpd v2.3 AMD64 binary and its existing hash, arguments, XML version, rate/byte/runtime limits, consent, target binding, and Job Object containment.
2. Create the scratch file only after fresh main-process consent and a fresh agent target/identity scan. Use `CREATE_NEW`, a cryptographically random exact name, fixed bounded aligned size, reparse-point rejection, one-link validation, captured volume serial/file ID, and a DACL granting access only to the current user and SYSTEM.
3. While DiskSpd runs, hold an ordinary owner handle with only read/write/attribute access, no DELETE access, and no `FILE_FLAG_DELETE_ON_CLOSE`. Its share mode is exactly `FILE_SHARE_READ | FILE_SHARE_WRITE` (no `FILE_SHARE_DELETE`), which allows DiskSpd's read/write open while blocking DELETE opens during the lease. The handle must not be inherited by the child. The test must prove the exact share matrix against the pinned engine in Windows CI.
4. Before the file can be used by DiskSpd, persist an atomic journal record under the current user's Local AppData. The journal directory and each record use an explicit current-user/SYSTEM DACL. The record contains a schema version, random lease ID, canonical volume-GUID root and root identity, exact scratch path, captured volume serial and 128-bit file ID, expected size, creation time, and state (`prepared`, `ready`, `running`, or `cleanup_pending`). Do not store raw storage identifiers, serial numbers, or renderer input.
5. Journal updates use a same-directory temporary record, flush the complete bytes, and atomically replace the old record. A run cannot start until the complete `ready` record is durable; immediately before launch the record transitions to `running` and is flushed.
6. Cancellation, timeout, output failure, shutdown, or child failure must terminate the complete Job Object and confirm it is empty before cleanup. If the job cannot be confirmed empty, retain the lease and journal, return a sanitized `cleanup_required` result, and block another run.
7. After drain, close the owner handle, then reopen the exact path with `DELETE | FILE_READ_ATTRIBUTES` and share mode zero. Compare the opened handle's final path, volume identity, file ID, regular-file attributes, link count, and exact expected size to the journal. Set delete disposition through that validated handle, close it, verify the exact path is absent, then remove the journal. Any sharing conflict or mismatch leaves the file and journal untouched and latches `cleanup_required`.
8. On startup, reconcile only entries from the protected journal directory. Never enumerate volume roots or delete by filename prefix, wildcard, or an unvalidated path. Verify the mounted volume identity and all captured file metadata again. If the volume is unavailable, defer and keep benchmark execution blocked until recovery can be completed.
9. If a crash leaves a `prepared` record without a committed file ID, or a malformed/corrupt record, do not infer ownership from the random name. Keep the target untouched, surface cleanup-required state, and require reviewed remediation. This explicitly accepts a rare recoverable leftover in exchange for never guessing which file to delete.
10. Keep `BENCHMARK_RUNTIME_ENABLED = false` and UI controls disabled until the revised lifecycle, recovery, tests, CI, result validation, packaging, and remaining P2.5 gates pass. User-owned physical Windows 11 acceptance remains separate and is not simulated by hosted CI.

## Required tests and gates

- Red/green sharing tests: delete-pending owner + pinned-DiskSpd-style open fails with error 32; ordinary owner with no DELETE access + fixed DiskSpd sharing succeeds.
- Pinned DiskSpd CI test on a disposable runner-temp file: exact binary/hash, fixed read-only workload, valid XML, job drain, post-run handle-bound cleanup, and file absence.
- Ordinary success, cancellation, timeout, primary-process crash, agent-process crash, and restart-recovery tests using isolated temporary roots and child fixtures only.
- No-delete assertions for invalid journal schema, bad ACL, missing or malformed identity, changed volume, replacement at the same path, wrong size, reparse point, hard link, unknown journal state, create-to-journal crash window, and un-drained Job Object.
- Journal permission, atomic-write, interrupted-replace, duplicate lease, idempotent recovery, and cleanup-required latch tests.
- Windows x64 full locked tests, rustfmt, warnings-as-errors Clippy, Electron main tests, build, mock-only UI smoke, npm/Rust audits, SBOM/package validation, and fresh GitHub Actions checks.
- No physical-device inventory, real-volume benchmark, or physical-media write by Codex. The user performs the Windows 11 acceptance.

## Explicit trade-off

This proposal removes the planned immediate OS delete-on-last-handle behavior because it is incompatible with the pinned engine. It replaces that guarantee with exact-object recovery on normal cleanup and a later application start. A crash can leave an owned scratch file temporarily; the application must block new runs while a known lease remains unresolved and must never perform prefix-based cleanup.
