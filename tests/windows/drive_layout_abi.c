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
#include <stddef.h>
#include <stdio.h>

#define ABI_CHECK(condition, label) _Static_assert((condition), label)

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
