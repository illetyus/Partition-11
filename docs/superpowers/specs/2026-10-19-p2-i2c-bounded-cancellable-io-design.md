# P2-I2c-7 — Bounded and cancellable metadata I/O design

Status: design approved in chat; implementation not started.

## Goal

Prevent the non-elevated Windows metadata diagnostic from waiting indefinitely
inside a device driver while preserving the existing fail-closed PnP behavior.

A metadata request must either complete, be cancelled, or be isolated and
terminated within a bounded time. No stale result may escape after a timeout,
PnP invalidation, worker failure, or cancellation race.

## Scope

This design applies only to the existing hard-coded, metadata-only IOCTL set:

- `IOCTL_STORAGE_GET_DEVICE_NUMBER`
- `IOCTL_DISK_GET_LENGTH_INFO`
- `IOCTL_STORAGE_QUERY_PROPERTY`
- `IOCTL_DISK_GET_DRIVE_LAYOUT_EX`

It does not add arbitrary IOCTLs, raw sector reads, writes, formatting,
mounting, repair, resize, elevation, persistent device selection, or renderer
control of a disk path.

Electron physical-disk capability flags remain disabled.

## Chosen architecture

The existing synchronous metadata execution will move behind a short-lived,
disposable Windows worker process. The worker owns the device HANDLE, its
per-HANDLE PnP notification subscription, each `OVERLAPPED` structure, the
completion event, and all I/O buffers.

The worker opens the internally selected SetupAPI path with
`FILE_FLAG_OVERLAPPED`. Each metadata IOCTL uses a dedicated
`OVERLAPPED` object and event. An operation that returns immediately is
handled as completed. An operation returning `ERROR_IO_PENDING` is waited on
with a bounded completion wait.

The parent agent remains the policy boundary. It performs fresh SetupAPI
enumeration, starts a worker for one enumerated target, enforces a total worker
budget, validates the post-worker inventory, and rejects any result if the
scan changed.

The worker is not a general disk service. It accepts one internal request,
executes the fixed metadata sequence, returns a bounded result, and exits.

## Time budgets

The initial fixed limits are:

- per metadata IOCTL completion budget: **5 seconds**
- cancellation completion grace period: **1 second**
- total budget for one disk worker: **20 seconds**

These values are security limits, not performance targets. A timeout is an
unavailable/error result, never partial success.

The parent owns the 20-second hard deadline. A worker that does not exit after
the normal cancellation path is terminated by the parent and its result is
discarded.

## Per-IOCTL lifecycle

For every metadata IOCTL:

1. Check the worker scan/PnP state before issuing I/O.
2. Create one **manual-reset** completion event and one zero-initialized `OVERLAPPED`; store that event in `OVERLAPPED.hEvent`.
3. Issue `DeviceIoControl` with that exact `OVERLAPPED`.
4. If the call completes immediately, validate the byte count and continue.
5. If it returns `ERROR_IO_PENDING`, wait for completion for at most
   5 seconds.
6. If PnP invalidates the target or the 5-second budget expires, call
   `CancelIoEx(handle, &overlapped)` for that exact request.
7. Keep the event, `OVERLAPPED`, input buffer, and output buffer alive until
   completion is observed.
8. After requesting cancellation, wait for at most 1 additional second.
9. Accept only a normal completion that occurred before invalidation and still
   passes the existing epoch/inventory checks.
10. Treat `ERROR_OPERATION_ABORTED` as cancelled, not as a metadata result.
11. If `CancelIoEx` reports `ERROR_NOT_FOUND`, resolve the race by querying
    the operation's final completion state; never assume either success or
    cancellation.
12. If the operation still has not completed after the grace period, stop
    trusting the worker. The parent hard deadline terminates that disposable
    worker and discards all output.

No `OVERLAPPED` object or I/O buffer may be reused or freed while Windows can
still reference it.

## PnP integration

The per-HANDLE notification behavior from P2-I2c-6 remains the source of
device-removal invalidation.

With overlapped I/O, a query-remove notification no longer needs to wait for a
synchronous `DeviceIoControl` call to return before cancellation can be
requested.

On `DEVICEQUERYREMOVE`:

- invalidate the worker's scan state;
- if no I/O is active, close the HANDLE in the callback and return success;
- if an overlapped request is active, mark that exact request for
  `CancelIoEx`, return `ERROR_CANCELLED` without blocking the callback,
  and let the worker's normal execution path issue cancellation and drain the
  completion;
- do not free the request buffers while cancellation is pending;
- close the device HANDLE only after the active request reaches a terminal
  completion state.

The notification callback never performs the 1-second cancellation wait. It
only changes atomic/request state and, when idle, performs the Windows-required
HANDLE close. This preserves the no-blocking-callback rule from P2-I2c-6.

`DEVICEQUERYREMOVEFAILED` does not resurrect the old worker. A later parent
scan re-enumerates and starts a new worker.

`DEVICEREMOVEPENDING` and `DEVICEREMOVECOMPLETE` invalidate the worker and
force the same cancel/drain/close path.

`CM_Unregister_Notification` remains outside notification callbacks.

## Worker boundary

The worker must not accept a renderer-supplied path or IOCTL code.

The parent launches the same trusted agent executable in an internal worker
mode and transfers the selected device path through a private inherited pipe,
not a command-line argument. This avoids exposing the device path in normal
process listings and prevents the worker mode from becoming a public disk API.

The worker protocol is versioned and one-shot. The request contains only the
minimum information required to identify the parent-selected interface. The
response contains only the bounded metadata fields already produced by the
diagnostic path plus an explicit terminal status.

Malformed, oversized, repeated, or unexpected worker messages fail closed.

The worker must run non-elevated. It must reject execution if the process token
is elevated, matching the current agent policy.

## Parent lifecycle

For each freshly enumerated disk interface:

1. Capture the parent scan generation and inventory snapshot.
2. Start one disposable worker.
3. Send only the parent-selected interface through the private pipe.
4. Wait for the worker under the 20-second total deadline.
5. If the worker exits normally, parse the bounded response.
6. If it exceeds the deadline, terminate the worker and discard its output.
7. Re-enumerate the parent inventory and validate the existing scan snapshot.
8. Accept the worker result only when the snapshot and generation are still
   current.

The parent never treats worker termination as successful cancellation.

## Failure semantics

The implementation must distinguish at least:

- normal metadata success;
- unsupported metadata query;
- access denied;
- device removed;
- PnP identity invalidation;
- per-IOCTL timeout;
- cancellation completed;
- cancellation race where I/O completed first;
- worker crash/abnormal exit;
- worker hard-timeout termination;
- malformed worker response.

Any timeout, cancellation, PnP invalidation, malformed response, worker crash,
or hard termination prevents that worker's metadata from being reported as
ready.

## Concurrency and lifetime rules

At most one metadata IOCTL is active per worker HANDLE at a time.

Each active request owns:

- one `OVERLAPPED`;
- one completion event HANDLE;
- immutable input bytes;
- mutable output bytes;
- returned-byte storage;
- a cancellation/terminal-state marker.

The request remains alive until Windows reports a terminal completion or the
entire worker process has been terminated. This is the central memory-safety
rule.

The worker may not issue the next IOCTL after cancellation, timeout, or PnP
invalidation.

## Testing strategy

Implementation follows TDD. Production behavior is added only after a test
fails for the intended missing behavior.

Deterministic Rust tests must cover:

- immediate successful completion;
- `ERROR_IO_PENDING` followed by success;
- 5-second timeout requests cancellation;
- cancellation completes with `ERROR_OPERATION_ABORTED`;
- completion wins the cancellation race;
- `CancelIoEx` returns `ERROR_NOT_FOUND` and final state is resolved;
- PnP invalidation cancels the active request and prevents later IOCTLs;
- request buffers are retained until terminal completion;
- 1-second cancellation grace expiry yields worker-failed state;
- 20-second parent budget terminates a non-responsive test worker;
- worker crash and malformed response are rejected;
- elevated worker execution is rejected;
- no arbitrary IOCTL or renderer-controlled path enters the worker protocol.

Windows CI must independently verify the SDK ABI/constants used by the FFI,
including `OVERLAPPED`, `FILE_FLAG_OVERLAPPED`, `ERROR_IO_PENDING`,
`ERROR_OPERATION_ABORTED`, `ERROR_NOT_FOUND`, and relevant wait results.

A hosted Windows test helper may simulate a worker that intentionally does not
respond so the parent hard-timeout path can be exercised without touching a
physical disk.

Existing full CI remains required:

- Rust safety tests on Windows;
- Rust safety tests on Ubuntu;
- Electron TypeScript/renderer build and IPC policy tests;
- Windows Electron UI preview;
- DiskSpd XML integration.

## Physical acceptance deferred from this design

Hosted CI cannot prove real driver cancellation behavior. Before physical disk
access is exposed to Electron, separate disposable-device Windows 11 tests are
still required for:

- USB surprise removal during every metadata query type;
- NVMe and SATA device removal/disable where practical;
- BitLocker volumes;
- 512e and 4Kn media;
- drivers that complete cancellation slowly;
- fast unplug/replug and temporary disk-number reuse.

These remain later acceptance gates and do not block implementing the bounded
worker architecture itself.

## Security invariants

This phase must preserve all of the following:

- no raw sector `ReadFile`;
- no `WriteFile`;
- no write/format IOCTL;
- no elevation request;
- no mount/repair/resize;
- no renderer-controlled device path;
- no renderer-controlled IOCTL;
- no persistent hardware identity minted from PnP path or disk number;
- no partial result after timeout/cancellation/PnP invalidation;
- all Electron physical-disk capability flags remain false.

## Documentation and rollout

Implementation should land in small reviewable PRs:

1. overlapped request primitive and deterministic cancellation tests;
2. per-HANDLE PnP integration with active overlapped request;
3. one-shot worker protocol and 20-second parent watchdog;
4. migration of the existing metadata sequence to the worker;
5. Windows SDK ABI additions and hosted worker timeout tests;
6. final documentation/handoff checkpoint after fresh 5/5 `main` CI.

Physical Windows 11 acceptance is a later, separate gate.

## References

- Microsoft: Synchronous and Asynchronous I/O
  https://learn.microsoft.com/en-us/windows/win32/fileio/synchronous-and-asynchronous-i-o
- Microsoft: CancelIoEx
  https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex
- Microsoft: GetOverlappedResultEx
  https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-getoverlappedresultex
- Microsoft: Registering for Notification of Device Interface Arrival and Device Removal
  https://learn.microsoft.com/en-us/windows-hardware/drivers/install/registering-for-notification-of-device-interface-arrival-and-device-removal
