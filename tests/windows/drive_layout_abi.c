/*
 * Independent Windows SDK ABI oracle for the pure Rust windows_layout parser.
 * Compiled as C11 by Microsoft's cl.exe against the installed Windows SDK.
 * No CreateFile, DeviceIoControl, device enumeration or disk I/O occurs.
 *
 * The byte offsets are explicitly used in crates/core/src/windows_layout.rs;
 * a Windows SDK/toolchain change must fail CI rather than silently reinterpret
 * real disk metadata.
 */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winioctl.h>
#include <setupapi.h>
#include <cfgmgr32.h>
#include <stddef.h>
#include <stdio.h>

#define ABI_CHECK(condition, label) _Static_assert((condition), label)


/* CM_Register_Notification interface filter: SDK C ABI versus Rust FFI.
 * Callback registration is non-mutating; the probe never registers anything. */
ABI_CHECK(sizeof(CM_NOTIFY_FILTER) == 416, "PnP interface filter x64 size changed");
ABI_CHECK(offsetof(CM_NOTIFY_FILTER, FilterType) == 8, "PnP filter type offset");
ABI_CHECK(offsetof(CM_NOTIFY_FILTER, u) == 16, "PnP GUID union offset");
ABI_CHECK(CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE == 0, "PnP interface filter enum changed");
ABI_CHECK(CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE == 1, "PnP handle filter enum changed");
ABI_CHECK(offsetof(CM_NOTIFY_FILTER, u.DeviceHandle.hTarget) == 16, "PnP handle target offset");
ABI_CHECK(CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL == 0, "arrival enum changed");
ABI_CHECK(CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL == 1, "removal enum changed");
ABI_CHECK(CM_NOTIFY_ACTION_DEVICEQUERYREMOVE == 2, "query-remove enum changed");
ABI_CHECK(CM_NOTIFY_ACTION_DEVICEQUERYREMOVEFAILED == 3, "query-remove-failed enum changed");
ABI_CHECK(CM_NOTIFY_ACTION_DEVICEREMOVEPENDING == 4, "remove-pending enum changed");
ABI_CHECK(CM_NOTIFY_ACTION_DEVICEREMOVECOMPLETE == 5, "remove-complete enum changed");
ABI_CHECK(ERROR_CANCELLED == 1223, "query-remove veto Win32 error changed");

/* OVERLAPPED metadata IOCTL request ABI and cancellation result values. */
ABI_CHECK(sizeof(OVERLAPPED) == 32, "Rust OVERLAPPED x64 size changed");
ABI_CHECK(offsetof(OVERLAPPED, hEvent) == 24, "Rust OVERLAPPED event offset changed");
ABI_CHECK(FILE_FLAG_OVERLAPPED == 0x40000000, "overlapped open flag changed");
ABI_CHECK(ERROR_IO_PENDING == 997, "pending I/O error changed");
ABI_CHECK(ERROR_IO_INCOMPLETE == 996, "incomplete I/O error changed");
ABI_CHECK(ERROR_OPERATION_ABORTED == 995, "aborted I/O error changed");
ABI_CHECK(ERROR_NOT_FOUND == 1168, "CancelIoEx race error changed");
ABI_CHECK(WAIT_TIMEOUT == 258, "wait timeout result changed");

/* SP_DEVINFO_DATA is used to obtain a PnP instance ID from the same set. */
ABI_CHECK(sizeof(SP_DEVINFO_DATA) == 32, "Rust DeviceInfoData layout changed");
ABI_CHECK(offsetof(SP_DEVINFO_DATA, ClassGuid) == 4, "PnP class GUID offset");
ABI_CHECK(offsetof(SP_DEVINFO_DATA, DevInst) == 20, "PnP devnode offset");
ABI_CHECK(offsetof(SP_DEVINFO_DATA, Reserved) == 24, "PnP reserved pointer offset");

ABI_CHECK(sizeof(PARTITION_INFORMATION_GPT) == 112, "GPT partition union arm changed");
ABI_CHECK(sizeof(PARTITION_INFORMATION_MBR) <= sizeof(PARTITION_INFORMATION_GPT), "MBR union cannot extend past GPT union");
/* MBR union-arm size is SDK-revision dependent; the parser reads only
 * its first PartitionType byte, which is checked through union offset. */
ABI_CHECK(sizeof(PARTITION_INFORMATION_EX) == 144, "Rust ENTRY_BYTES must be updated");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, PartitionStyle) == 0, "entry style offset");
/* Some SDK reference pages show PartitionOrdinal; the x64 Windows SDK
 * headers on the CI runner omit it. The parser does not read this field. */
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, StartingOffset) == 8, "entry start offset");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, PartitionLength) == 16, "entry length offset");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, PartitionNumber) == 24, "entry number offset");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, RewritePartition) == 28, "entry rewrite offset");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, IsServicePartition) == 29, "entry service offset");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, Gpt) == 32, "entry GPT union offset");
ABI_CHECK(offsetof(PARTITION_INFORMATION_EX, Mbr) == 32, "entry MBR union offset");
ABI_CHECK(sizeof(DRIVE_LAYOUT_INFORMATION_GPT) == 40, "drive GPT union arm changed");
ABI_CHECK(offsetof(DRIVE_LAYOUT_INFORMATION_EX, PartitionStyle) == 0, "layout style offset");
ABI_CHECK(offsetof(DRIVE_LAYOUT_INFORMATION_EX, PartitionCount) == 4, "layout count offset");
ABI_CHECK(offsetof(DRIVE_LAYOUT_INFORMATION_EX, PartitionEntry) == 48, "Rust HEADER_BYTES must be updated");
ABI_CHECK(sizeof(DRIVE_LAYOUT_INFORMATION_EX) == 192, "header plus one partition entry changed");
ABI_CHECK(IOCTL_DISK_GET_DRIVE_LAYOUT_EX == 0x00070050, "metadata IOCTL number changed");
ABI_CHECK(PARTITION_STYLE_MBR == 0 && PARTITION_STYLE_GPT == 1 &&
          PARTITION_STYLE_RAW == 2, "partition style values changed");

int main(void) {
    puts("Windows SDK partition layout ABI verified: header=48 entry=144 GET=0x00070050");
    return 0;
}
