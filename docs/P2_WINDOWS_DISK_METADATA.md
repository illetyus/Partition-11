# P2-I2b — Windows metadata-only agent probe (first increment)

Status: **development-only diagnostic**. Tracking: [Issue #22](https://github.com/illetyus/Partition-11/issues/22), parent [#19](https://github.com/illetyus/Partition-11/issues/19).

## Scope and security

- Uses only a newly enumerated list of present SetupAPI disk-interface paths.
- Only the private Rust agent opens the returned paths, with **GENERIC_READ**, OPEN_EXISTING and READ/WRITE/DELETE sharing; there is no GENERIC_WRITE, WriteFile, raw-sector ReadFile, mount, UAC, renderer-supplied path or Electron IPC connection.
- Refuses an elevated access token before any physical device handle is opened; token query failure is fail-closed.
- Each transient handle is RAII-closed. No PnP paths, device identifiers, bus data or capacity values are exposed to the UI; a **local CLI diagnostic** prints aggregate counts only.
- Supported bounded IOCTLs: STORAGE_GET_DEVICE_NUMBER, DISK_GET_LENGTH_INFO, STORAGE_QUERY_PROPERTY for AccessAlignment and Device descriptors. Returned byte counts, descriptor versions/sizes and sector geometry are independently validated. Unsupported fields remain unavailable.
- Device number is not a stable identifier. A second query on the same handle and a second SetupAPI enumeration invalidate some obviously removed/reassigned devices, **but these checks do not solve TOCTOU, path reuse, or stable identity**.
- Four Electron capability flags remain false. No benchmark execution, partition layout/sector reads, write features or formatting are implemented.

## Run the local diagnostic

On a **non-elevated Windows** prompt, build the agent with `cargo build --locked -p partition11-agent` and run `partition11-agent metadata-summary`. It emits JSON counts with `"mode":"diagnostic-only"` and `"ui_disk_access_enabled":false`. The existing `discovery-count` diagnostic still opens no disk. Error exit code 3 denotes rejected security policy, device discovery failure or inaccessible token query.

Running as administrator explicitly fails; do **not** rerun elevated to bypass access denied. Some disks or fields may be inaccessible to non-elevated callers; their counts are not guessed.

## Validation and caveats

- Rust pure-parser tests: bad/truncated device-number, length, alignment/bus descriptor; Win32 denied/removal/unsupported classification; temporary number mismatch.
- CI on Windows validates compilation and tests on a GitHub-hosted Windows VM; Linux checks leave the Windows module out. No user disk writes occur in CI.
- **Not yet independently hardware-verified**: real Windows 11 NVMe/SATA/USB, legacy GPT/MBR, 512e/4Kn, BitLocker, dynamic/removable media, hot-unplug under load, access-denied device handles, or systems with UAC disabled.
- Synchronous IOCTL driver timeout behavior is not yet bounded by a process-level watchdog. This command is diagnostic-only and not remotely invokable.
- The query **does not** return partition layout, mapped volumes, filesystem content, or user-facing disk metadata. Those remain open in #22 and the later P2-I2c/I4/I5/I6 phases.

## Required hardware acceptance matrix (future)

| Device/state | Checks to record | Safety outcome |
|---|---|---|
| NVMe, SATA SSD and SATA HDD | total length, bus type, logical and physical sector sizes | compare with trusted Windows hardware UI |
| USB removable / surprise disconnect | disappearance during first/second enumeration and IOCTL | invalidate result, no hangs/use-after-close |
| 512e and 4Kn | 512/4096 logical/physical and physically native 4096/4096 | reject impossible or unknown fields |
| GPT and MBR | separate future drive layout query/partition IDs | no layout claimed by current diagnostic |
| BitLocker, system/boot and locked disk | missing/access-denied fields, token behavior | no elevation, never unlock or modify |
| Uninitialized, offline, virtual, zero-capacity and corrupt device | unsupported/zero/bad descriptor and error codes | explicitly unavailable; never fabricate a size |
| Device-number/path reuse | remove/insert between enumerations | require later cryptographically opaque identity snapshot; current result is provisional |

Microsoft API references:
- https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_storage_get_device_number
- https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-ioctl_disk_get_length_info
- https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_access_alignment_descriptor
- https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-storage_device_descriptor
- https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation

## P2-I2b drive-layout parser, second increment (2026-10-18)

- New **pure** `crates/core/src/windows_layout.rs` parses a bounded x64/ARM64
  `DRIVE_LAYOUT_INFORMATION_EX` metadata buffer. Maximum 128 partition slots,
  48-byte header and 144-byte Windows layout entry; no disk I/O or UI wiring.
- Validates MBR/GPT/RAW style, declared count and exact bounded spans, disk
  capacity, available logical sector alignment, nonzero occupied ranges,
  distinct partition numbers and overlapping extents. Empty MBR slots are
  ignored and sorting is by byte offset.
- Synthetic tests cover GPT/MBR, empty RAW, malformed/truncated counts,
  overlaps, duplicate numbers, negative/wrapped/out-of-bounds positions and
  sector misalignment. Native Windows SDK ABI conformance and genuine device
  layout queries remain **future work**: this parser is not yet fed by a live
  `DeviceIoControl` call.
- No new capabilities. `inspectImages`, `inspectPhysicalDisks`, writes and
  formatting remain disabled. Next PR: bounded IOCTL and native ABI checks,
  strict fresh-handle identity comparisons, and physical hardware QA.
