# ADR-0003: Microsoft DiskSpd file-targeted benchmarking

Status: ACCEPTED for planning, implementation NOT STARTED (2026-10-17).

## Context

Partition 11 needs a disk benchmark screen without weakening disk-format
protection. DiskSpd supports both ordinary file targets and dangerous raw
device/partition targets; its write workloads may erase real data without
a warning.

## Decision

Add P2.5 after P2 and before P3. Use Microsoft DiskSpd v2.3 (MIT) as a
separate, non-elevated Windows benchmark engine with only verified,
application-owned test-file targets, fixed presets and structured XML
results. Use CrystalDiskMark solely as workload/UX reference; fio remains
an optional external GPLv2 differential test oracle.

Build mock-only UI and XML parsing first. Do not enable filesystem-file
writes until target-ownership guard, cancellation, and disposable-media
Windows tests pass. Raw disk writing remains restricted to future P5.

## Consequences

Maintains Electron/Rust separation and MIT distribution direction.
Supports usable MB/s, IOPS and latency measures; test results reflect
Windows filesystem + storage stack rather than native unmounted ext4
throughput. Packaging and version tracking must be independently audited.

Details: ../P2_5_DISK_BENCHMARK.md
