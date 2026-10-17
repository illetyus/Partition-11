# P2 — Read-only MBR/GPT partition table parser

Status: **memory-only prototype**; no Windows disk enumeration has been
implemented. No raw disk handles or filesystem data are accessed.

## Decoder contract

`partition::parse_mbr` receives one 512-byte or 4096-byte logical sector
and a verified total device-sector count, validates the 0x55AA signature,
entry bounds and boot flags, and returns at most four **primary** entries.

`partition::parse_gpt_header` verifies the GPT signature, revision, header
CRC32 (IEEE), geometry, array offset and bounded allocation sizes. It
requires the expected physical header LBA. Header validation is not proof
that the partition array is correct.

`partition::parse_gpt_entries` validates the array CRC32, occupied entry
bounds, unique GUID presence, and rejects overlapping used entries. It
preserves GUIDs as raw on-disk bytes.

## Explicit non-features

- No OS device enumeration, raw I/O, UAC elevation, or write operations.
- No extended/logical MBR traversal, GPT names, or filesystem inspection.
- No recovery from damaged GPT primary header using backup copies.
- No guarantee that synthetic tests establish conformance with real disks.
- No UI disk listing yet; capability flags must remain disabled.

## Before enabling physical-device discovery

Build a separate read-only Win32 adapter with an authenticated target
snapshot and verify byte offsets, sector sizes, hot-plug identity,
genuine 512e/4Kn images, GPT primary/backup consistency, malformed
partition arrays and OS-level access-denied behavior. Cross-check
sample images against Linux `sfdisk`/`sgdisk` and Windows disk APIs.
