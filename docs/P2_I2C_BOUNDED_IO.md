# P2-I2c — Bounded and cancellable Windows metadata I/O

## Status

[PR #46](https://github.com/illetyus/Partition-11/pull/46) merged to
`main` at `a9cb53344ff560082096d728eb453af0532a7415` after five passing
PR checks. The fresh post-merge `main` checks also passed **5/5** at that
SHA. The implementation is complete at code/CI level; physical-device
acceptance and user-facing disk access remain separate gates.

## Request lifecycle

The parent agent retains SetupAPI enumeration, scan-generation state, and
final inventory validation. It launches one disposable child per disk. The
child rejects elevated execution, freshly validates the exact path/instance
pair selected by the parent, opens the handle for metadata-only overlapped
I/O, and accepts only the fixed set of four metadata IOCTLs. The private
worker protocol contains no IOCTL selector and is not an Electron IPC.

Each IOCTL has a five-second monotonic budget recorded **before** calling
`DeviceIoControl`. A timeout or PnP invalidation requests `CancelIoEx` for
that exact `OVERLAPPED` request. The worker allows one second for completion
and resolves races from the final completion state. It never frees or reuses
an event, `OVERLAPPED`, handle, or buffer while Windows may still reference
it.

The worker arms its own 20-second termination timer before reading input or
opening a device. The parent independently enforces a 20-second deadline,
discards all output after timeout, and quarantines the child and its pipe
threads until they are reaped. Another worker does not launch while that
quarantine remains. The parent accepts a normal reply only if the process
exited successfully, the response is bounded and valid, and the parent scan
generation and inventory remain current.

Timeout, cancellation, worker failure, malformed data, or identity changes
produce an unavailable result; no partial metadata is treated as current.
The query path remains read-only and non-elevated.

## Verification

PR head `4172f5e970bab5668d520563796efc1556c774f9` passed all five required
checks: Rust safety tests on Ubuntu and Windows, Electron TypeScript/renderer
build and IPC policy tests, Windows Electron UI preview, and DiskSpd XML
integration. The final post-merge `main` SHA
`a9cb53344ff560082096d728eb453af0532a7415` passed:

- [CI — Rust Ubuntu/Windows and Electron TypeScript](https://github.com/illetyus/Partition-11/actions/runs/39195989162)
- [Windows Electron UI preview](https://github.com/illetyus/Partition-11/actions/runs/39195989210)
- [DiskSpd XML integration](https://github.com/illetyus/Partition-11/actions/runs/39195989174)

The Windows UI run includes a real Electron smoke test and an unsigned,
unpacked preview package. These hosted Windows runner checks are not physical
Windows 11 hardware acceptance.

## Remaining gates

- Qualify durable hardware-backed identifiers and fast unplug/replug races.
- Test removal and hotplug on physical Windows 11 USB, NVMe, and SATA devices,
  including BitLocker, 512e, and 4Kn scenarios.
- Design and validate authenticated, read-only agent IPC before adding any
  disk-list UI or renderer access.
- Keep all physical-disk, raw-sector, write, format, mount, repair, resize,
  and elevation capabilities disabled until their own safety gates pass.

These remaining items stay open in [Issue #31](https://github.com/illetyus/Partition-11/issues/31).
