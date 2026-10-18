# P2-I2a — Windows native disk interface discovery

Status: separate agent diagnostic only; no Electron/IPC integration. This
slice is narrower than the full P2-I2 acceptance in GitHub Issue #19.

## Implementation

- Use **SetupAPI**: GUID_DEVINTERFACE_DISK plus DIGCF_PRESENT |
  DIGCF_DEVICEINTERFACE. This enumerates present disk-class interfaces
  including ones without mounted volume drive letters.
- Gather each opaque, internal Windows UTF-16 device interface path via
  bounded, two-pass SetupDiGetDeviceInterfaceDetailW. Max 256 interfaces and
  32 KiB bytes per path result. Always release the device-information set.
- Zero device opens: no CreateFileW, no DeviceIoControl, no access elevation,
  no partition reads and no write flags anywhere in this adapter.
- Diagnostic command: `partition11-agent discovery-count` (Windows only).
  Emits **count only**, not raw interface paths/serials, with mode explicitly
  `diagnostic-only`. Errors are limited to API operation and Win32 code.
- `capabilities` remains four false flags; the GUI is not connected.
- FFI code lives only in disk_discovery.rs; Rust agent root denies unsafe
  elsewhere. No dependencies or Cargo.lock modifications needed.

## Verification

- Windows and Linux cargo fmt, clippy with warnings denied, and unit tests.
- Windows unit test really calls SetupAPI from GitHub Windows runner and
  asserts result is bounded, unique and contains valid nonempty paths.
- Validation tests reject malformed/missing-NUL/oversized detail buffers.
- Do not claim this test is physical Windows 11 or hot-plug validation.

## Known limits / required next steps (P2-I2b onward)

1. These interface paths are *not stable disk identities*. Media removal,
   restart, PnP reassignment, symlinks/reparse or duplicates need future
   revalidation. They are never accepted from Electron.
2. Native discovery here is just **presence**: no physical disk length,
   logical/physical sectors, partitions, bus/model/serial or EXT metadata.
3. Later read-only metadata queries should use documented Win32 APIs and
   purpose-built handle/IOCTL wrappers with no permissions to write or
   mount. Do not infer approval from a "read-only" method name.
4. Query denial and unsupported/corrupt devices must be returned as
   structured errors; a different disk inserted after discovery invalidates
   the selection.
5. Add real 512e/4Kn, GPT/MBR, USB hotplug, BitLocker and physical Windows 11
   tests before any capability flags become true.

Sources:
- https://learn.microsoft.com/en-us/windows-hardware/drivers/install/guid-devinterface-disk
- https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdigetclassdevsw
- https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdigetdeviceinterfacedetailw
- https://learn.microsoft.com/en-us/windows-hardware/drivers/install/enumerating-installed-device-interface-classes
