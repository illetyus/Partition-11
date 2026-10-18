# P2-I2c-5: fail-closed metadata query epoch (Issue #31)

Status: agent-only incremental implementation under review; no user-facing disk access.

## Problem and scope

Windows PnP interface notifications can invalidate the agent's private
`ScanGeneration` while a synchronous, metadata-only Win32 call is executing.
The previous scan-level validation discarded the final scan result, but
unnecessarily continued probing other properties/devices before that check.

This slice binds one `ProbeEpoch` to the same `ScanGeneration` already used
by `pnp_notifications.rs` and `scan_identity.rs`.

- Fail **before** opening a discovered disk path if the epoch changed.
- Fail **after** opening the handle if the epoch changed during `CreateFileW`.
  The newly opened handle is RAII-closed without querying it.
- Validate before and after each fixed metadata-only IOCTL. If a notification
  raced with the call, the epoch error overrides both success and an optional
  field's unavailable/error result.
- For bounded, retrying drive-layout queries, validate the epoch on **each**
  iteration so a previously cancelled scan does not issue further requests.
- Fail before the following device probe, after final inventory enumeration,
  and again after notification unregister; preserve whole-scan rejection.
- Tests first established the absent `ProbeEpoch` (Windows CI E0433), then
  exercise cancellation before a closure, during a successful closure and
  during an otherwise-unsupported closure.

## Explicitly NOT solved

- A driver-blocked **synchronous** `DeviceIoControl` is not interrupted; the
  check runs only when it returns. This is not bounded I/O cancellation.
- There is still **no per-device HANDLE query-remove subscription**.
  Per Microsoft, responding to `CM_NOTIFY_ACTION_DEVICEQUERYREMOVE` must
  release open device handles, and the reopen-after-query-remove-failed case
  needs its own lifecycle handling. Closing a handle from a callback while
  another thread is using it without coordination is unsafe.
- PnP interface paths, instance IDs and Windows disk numbers are not trusted
  long-term hardware identities. Fast replug and controller changes still
  require an independent identity/handle lifecycle design.
- No Electron IPC, `inspectPhysicalDisks`, real hardware scan, raw sector
  read/write, elevation, benchmark, formatting, mount, repair or resize is
  activated.

## Subsequent gates

1. Formalize handle-specific query-remove lifecycle and synchronous I/O
   interruption/timeout policy (separate design and tests before FFI changes).
2. Evaluate device identifier strength/absence and ambiguous identical-model
   devices, not merely PnP devnode labels.
3. Test real Windows 11 USB unplug/replug, NVMe/SATA, 512e/4Kn, BitLocker,
   access denied and query-remove races on disposable hardware.
4. Preserve PR 5/5 CI, squash merge, recheck new `main` CI and update
   Issue #31 and the session handoff.

Official references:
- https://learn.microsoft.com/en-us/windows-hardware/drivers/install/registering-for-notification-of-device-interface-arrival-and-device-removal
- https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/ne-cfgmgr32-cm_notify_action
- https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_unregister_notification
