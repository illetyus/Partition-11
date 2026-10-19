# P2-I2c-6 — per-HANDLE removal lifecycle

Status: implemented in the non-elevated diagnostic agent only. No Electron disk
capability is enabled.

## Windows contract

After a device interface is opened, Partition 11 registers a second
`CM_Register_Notification` subscription with
`CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE` and the exact agent-owned HANDLE.
The independent Windows SDK C oracle checks the x64 filter size, HANDLE union
offset, filter enum and query/remove action values.

The HANDLE remains valid for the entire registration call. A notification that
races registration cannot close the filter HANDLE; query-remove is vetoed and
the HANDLE is released immediately after registration returns.

## Lifecycle

- **DEVICEQUERYREMOVE, idle HANDLE:** invalidate the scan, close the HANDLE in
  the callback and return `ERROR_SUCCESS`, allowing removal.
- **DEVICEQUERYREMOVE, synchronous I/O owns HANDLE:** invalidate the scan,
  request deferred close and return `ERROR_CANCELLED`. The HANDLE is closed
  as soon as that synchronous call returns. This avoids closing a Win32 HANDLE
  underneath `DeviceIoControl`.
- **DEVICEREMOVEPENDING / DEVICEREMOVECOMPLETE:** invalidate the scan and close
  immediately when idle. If a synchronous call owns the HANDLE, mark it for
  close and release it immediately after that call returns.
- **DEVICEQUERYREMOVEFAILED:** never resurrect the now-stale diagnostic.
  Microsoft permits reopening to continue I/O; Partition 11 instead rejects
  the scan. A later invocation performs fresh SetupAPI enumeration and opens a
  new HANDLE, eliminating stale path/number reuse within the old scan.
- Notification unregister happens outside the callback. The owner calls
  `CM_Unregister_Notification`, which waits for outstanding callbacks before
  their context can be released.

## Explicit remaining gates

This slice does **not** forcibly interrupt a driver-blocked synchronous
`DeviceIoControl`. Therefore a query-remove can be temporarily vetoed until
that call returns. Bounded/cancellable I/O is the next separate item.

It also does not establish durable hardware identity, enable renderer IPC,
perform raw sector reads/writes, elevate, format, mount, repair or resize.
Hosted GitHub Windows runners verify compilation, ABI and deterministic state
transitions; physical Windows 11 USB/NVMe/SATA/BitLocker/512e/4Kn hotplug
acceptance remains separate.

References:
- https://learn.microsoft.com/en-us/windows-hardware/drivers/install/registering-for-notification-of-device-interface-arrival-and-device-removal
- https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/ns-cfgmgr32-cm_notify_filter
- https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/ne-cfgmgr32-cm_notify_action
- https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_unregister_notification
