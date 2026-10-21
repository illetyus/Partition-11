# ADR-0004: Recoverable scratch leases for pinned DiskSpd

## Status

Accepted by the user on 2026-10-21 for implementation. Runtime enablement and physical-device testing remain separately gated.

## Date

2026-10-20

## Context

The approved P2.5 design asks the Windows CI gate to prove that pinned Microsoft DiskSpd v2.3 can open a temporary file while the owning lease uses delete-on-close. PR #53 reached the gate at `cf2f3da7196b5251fd25ce1d4f1028274fe2d103`; DiskSpd exited with Windows error 32 (`ERROR_SHARING_VIOLATION`).

The Microsoft DiskSpd v2.3 source opens an existing target with `FILE_SHARE_READ | FILE_SHARE_WRITE`, without `FILE_SHARE_DELETE` ([pinned upstream source, lines 1693–1698](https://github.com/microsoft/DiskSpd/blob/5e7025bfc9d1364f185d4c30963d7ac79195435e/IORequestGenerator/IORequestGenerator.cpp#L1693-L1698)). Microsoft documents that file-share compatibility is checked against every existing handle and that delete-pending files reject later opens unless delete sharing is compatible ([CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)). The current ScratchLease also requests DELETE access while DiskSpd's new handle does not share delete, so the existing primitive conflicts even before delete-on-close is added.

Local Win32-only disposable-file probes reproduced the conflict without launching DiskSpd: a delete-on-close handle followed by a DiskSpd-shaped open failed with error 32, even when the first handle did not explicitly request DELETE access. The intended ordinary owner handle (read/write access, `FILE_SHARE_READ | FILE_SHARE_WRITE`, no DELETE access) allowed the DiskSpd-shaped open. DELETE opens failed with error 32 while either that owner handle or the DiskSpd-shaped handle remained open. After both closed, an exclusive DELETE handle set `FILE_DISPOSITION_INFO`, and the file disappeared.

## Decision

Retain the exact pinned DiskSpd executable and replace the planned delete-on-close integration plus the current DELETE-access-in-flight owner handle with a user-protected, exact-object crash-recovery journal. During DiskSpd execution, the owner handle must not request DELETE and the file must not be delete-pending. The lease must still be created with CREATE_NEW, a cryptographically random name, a restrictive current-user/SYSTEM DACL, and captured volume/file identity. While active, the owner handle requests only read/write/attribute access and uses `FILE_SHARE_READ | FILE_SHARE_WRITE` without `FILE_SHARE_DELETE`. This permits DiskSpd's fixed read/write open but blocks DELETE opens while the owner handle remains open.

Before launching DiskSpd, atomically persist a per-user journal record containing a schema version, lease ID, canonical volume-GUID root and root identity, exact scratch path, captured volume serial and 128-bit file ID, expected size, and lifecycle state. The journal directory and records must have a DACL limited to the current user and SYSTEM. No serial number, stable device identifier, or renderer-provided path is stored.

After normal completion, cancellation, or timeout, the worker must verify that the Job Object has drained, close the owner handle, then open the exact path with `DELETE | FILE_READ_ATTRIBUTES` and share mode zero. It revalidates the volume, file ID, final path, regular-file metadata, link count, and expected size through this exclusive handle, marks only that handle for deletion, closes it, confirms the exact path is absent, and removes the journal record. If another handle wins the close-to-reopen race, cleanup fails closed; a replacement at the same path fails the identity check. An uncertain drain or cleanup keeps the journal and latches `cleanup_required`; it cannot report success or start another benchmark.

On application startup, recovery may inspect journal records only. It must never scan a target volume for a prefix or wildcard. A record with a complete identity can authorize deletion only after every identity and metadata check succeeds. A partial or corrupt record, replacement file, changed volume, wrong size, reparse point, multiple link, unavailable volume, or sharing conflict fails closed and keeps the run gate closed. If a crash occurs after file creation but before the exact file ID is durably journaled, automatic deletion is forbidden; the app must report cleanup required and preserve the file for reviewed remediation.

## Alternatives considered

### Keep delete-on-close and use the pinned DiskSpd binary

Rejected: the binary's fixed share mode conflicts with the existing delete-pending handle. The CI run and local Win32 probe both return error 32.

### Rebuild or patch DiskSpd to add FILE_SHARE_DELETE

Rejected for this slice: it changes the approved pinned upstream executable and hash, creating a separate provenance, maintenance, and release decision.

### Use an ordinary file and delete only on clean exit without recovery state

Rejected: an agent or app crash can leave an untracked scratch file, and a later cleanup cannot prove ownership by name alone.

## Consequences

- Successful-run and cancellation cleanup remain exact-handle operations after the benchmark process tree drains.
- Crash cleanup becomes recoverable on the next application start or when the volume is available again, rather than immediate at process exit. A crash in the create-to-journal window intentionally blocks instead of guessing.
- Unresolved journals can prevent new benchmark runs until the exact lease is safely resolved.
- `BENCHMARK_RUNTIME_ENABLED` remains false, and Start/Cancel remain disabled until the revised design, tests, CI, packaging, and all other P2.5 gates pass.
- Physical-device tests remain the user's Windows 11 acceptance gate.

## References

- [Microsoft CreateFileW sharing and delete-on-close semantics](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
- [Pinned Microsoft DiskSpd v2.3 target open](https://github.com/microsoft/DiskSpd/blob/5e7025bfc9d1364f185d4c30963d7ac79195435e/IORequestGenerator/IORequestGenerator.cpp#L1693-L1698)
- [P2.5 scratch-recovery specification](../superpowers/specs/2026-10-20-p2-5-scratch-recovery-journal.md)
- [P2.5 scratch-recovery implementation plan](../superpowers/plans/2026-10-20-p2-5-scratch-recovery-journal.md)
