# P2.5 — Disk Benchmark: literature review and implementation plan

Reviewed: 2026-10-17. State: P2.5A benchmark UI and typed, inert IPC in development. No benchmark worker is implemented.

## Decision and scope

Integrate Microsoft's **DiskSpd v2.3** as a non-elevated Windows executable
with narrowly pre-defined **file-backed** workloads. It is MIT licensed.
The accepted Partition 11 two-column Windows 11 layout gains a Benchmark tab.
A benchmark must never open a raw disk/partition for writing.

File-based throughput measures the Windows filesystem, controller, operating
system and media together. It does not directly measure performance of an
unmounted ext4 partition, nor prove how that partition behaves on Linux.

## Verified sources and alternatives

| Project | Evidence | Role |
|---|---|---|
| Microsoft DiskSpd | https://github.com/microsoft/diskspd — LICENSE MIT, README v2.3 release notes, x64/ARM64 assets, XML output | Selected benchmark engine |
| CrystalDiskMark | https://github.com/hiyohiyo/CrystalDiskMark — DiskBench.cpp MIT header and DiskSpd executable invocation | Workloads and UX, no code/assets copied |
| fio | https://github.com/axboe/fio — COPYING GPLv2, README.rst Windows/Cygwin build | Independent development oracle only |
| Windows File I/O | https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering | Unbuffered I/O alignment and cache rules |

Core DiskSpd documentation:

- https://github.com/microsoft/diskspd/wiki/Command-line-and-parameters
- https://github.com/microsoft/diskspd/wiki/Customizing-tests
- https://github.com/microsoft/diskspd/wiki/Analyzing-test-results
- https://github.com/microsoft/diskspd/wiki/XML-profiles

DiskSpd 2.3 adds IoRing and BypassIO (on compatible Windows 11 setups);
these remain advanced experimental options, excluded from P2.5 baseline.
Its P/E-core affinity and buffer changes can affect cross-version score
comparability, so version and test conditions must be recorded.

## Implementation slices

### P2.5A — Benchmark UI and mock-only backend

- Use the existing left-side protected disk/volume chooser.
- Add **Biçimlendir / Benchmark** screen navigation; preserve accepted styling.
- Right-hand pane: read/write figures, workload labels, target information,
  test size/duration, start/cancel, progress and expandable IOPS/latency.
- Implement typed benchmark presets and a versioned IPC contract.
- Parse genuine and synthetic DiskSpd XML into a normalized result schema.
- No arbitrary paths, target strings, command switches or XML from renderer.
- Mock values must be clearly identified and the Start button disabled.
- Windows Actions: screenshot and accessibility smoke tests; synthetic XML
  and malformed/truncated/oversized parser fixtures.

### P2.5B — Strictly file-backed live tests

- P2 read-only Windows discovery must supply verified stable volume identity
  and logical/physical sector sizes.
- User explicitly selects an eligible **non-system** writable filesystem
  volume with enough free capacity. Do not benchmark boot/recovery volumes.
- Create a private, random-named benchmark file using create-new/exclusive
  semantics. Never operate on an existing user file.
- Validate file handle, volume, reparse-point/link policy, canonical
  location and sector alignment before starting DiskSpd.
- Only use an allowlisted DiskSpd executable whose version and SHA-256
  match the pinned third-party inventory. Run directly without shell.
- Invoke only approved parameter combinations with an explicit single
  application-owned test-file target; refuse raw disk/volume specifiers.
- Ask for separate consent for write tests, stating estimated bytes written,
  runtime, free-space requirements and flash wear/thermal considerations.
- Bound every test's file size, duration, repetitions and concurrency.
- Parse bounded XML results, block DTD/external entities, report failures.
- Cancel by terminating the **complete owned process tree**; do not leave
  workloads running. Remove only the verified application-owned test file.
- Crash recovery may delete abandoned test files only after identity checks,
  never by wildcard directory recursion.

### P2.5C — Optional future enhancements

- Comparative history, export to JSON/CSV, sustained-write curves,
  read-only optional SMART/temperature telemetry, advanced IoRing/BypassIO
  profiles after compatibility testing; no baseline performance promises.
- ARM64 runtime and packaging must have their own CI acceptance gate.

## Initial workload presets (proposals, not implemented)

| Label | Access pattern | I/O block | Queue/thread | Metrics |
|---|---|---|---|---|
| SEQ1M Q8T1 | Sequential | 1 MiB | 8 / 1 | MB/s, IOPS, latency |
| SEQ1M Q1T1 | Sequential | 1 MiB | 1 / 1 | MB/s, IOPS, latency |
| RND4K Q32T1 | Random | 4 KiB | 32 / 1 | MB/s, IOPS, latency |
| RND4K Q1T1 | Random | 4 KiB | 1 / 1 | MB/s, IOPS, latency |

Run each row separately for **read and write** and label the direction.
Initial candidate standard settings: 1 GiB scratch file, 5 s warmup,
15 s measured test per workload/direction, one repetition. Final defaults
require empirical calibration; use a larger file for cache-sensitive SSD
measurements and show test size. An optional shorter Quick preset may be
introduced only if reliability is documented.

Results show **MB/s decimal** (1 MB = 1,000,000 bytes); do not confuse
with MiB/s. Include IOPS, avg/p50/p95/p99 latency where enough samples
exist, engine version, filesystem, cache mode (e.g. unbuffered -Su versus
write-through -Suw), test data compressibility, volume, CPU/power plan,
queue depth, test duration and timestamp.

DiskSpd output -Rxml and -L latency data are the canonical measurements;
an example CrystalDiskMark UI does not ensure score equivalence if workload
sizes, caching and queue depth differ.

## No-go and safety gates

1. Raw PhysicalDrive, raw partition and drive-letter-only DiskSpd targets
   must be rejected before any process launch.
2. Running benchmark writes on the system/boot disk is not supported in
   this phase; no UAC automatic elevation.
3. Never create benchmark files via a user-supplied arbitrary filename
   or run destructive DiskSpd -c / -w modes against existing user data.
4. Worker restricts all commands to fixed schemas; benchmark output is
   not evaluated as executable code or rendered as arbitrary HTML.
5. Cancellation, disk removal, insufficient free space, unexpected file
   identity and temperature/error thresholds must fail closed.
6. Disposable-drive Windows integration tests must verify that no existing
   file, raw disk or partition metadata changes.
7. GitHub Actions Windows virtual disks may verify functionality but
   **must not** be represented as consumer hardware benchmark results.
8. Do not redistribute binaries before full MIT notices, SBOM, release
   pinning and integrity checks are completed.

## Acceptance checklist

- [ ] P2 Windows read-only volume enumeration complete
- [ ] P2.5A typed profiles, UI and XML parser, deterministic fixtures
- [ ] P2.5A keyboard, dark/light and screenshot tests in Windows Actions
- [ ] P2.5B pinned DiskSpd executable and license notices
- [ ] P2.5B create-new scratch file, volume/ownership verification
- [ ] P2.5B raw targets, symlinks, UNC paths and injections rejected
- [ ] P2.5B bounded space/wear, cancellation and crash cleanup
- [ ] P2.5B SSD/HDD/USB disposable-device validation
- [ ] P2.5B repeatable reports with known cache and units
- [ ] Explicit review before any live write benchmark is enabled

## Planned boundaries

- apps/desktop or src/renderer/BenchmarkPanel.tsx — presentation only
- src/shared/benchmarkProtocol.ts — versioned typed IPC schema
- crates/benchmark — workload allowlist, reporting, XML parse and policy
- crates/benchmark-worker — non-elevated process manager/target validator
- tests/benchmark-fixtures — synthetic DiskSpd XML and malformed samples
- .github/workflows/windows-benchmark.yml — disposable file-only tests

## P2.5A implemented in the first preview PR

The existing disk sidebar now includes an independent Benchmark workspace
with four proposed fixed DiskSpd profiles, separate read/write placeholders,
draft test-size/repetition settings and expandable IOPS/latency details.
The UI deliberately shows **no invented scores** and disables Start and Cancel.

A versioned IPC availability query reports only that benchmarking is disabled.
No DiskSpd binary is distributed; no benchmark worker, arbitrary paths, shell
commands, raw device access, or actual disk I/O are present. Owned test-file runtime, cancellation, verified performance data, and physical
drive acceptance are still open implementation gates.

## P2.5A.2 — DiskSpd XML result decoding

A strict single-run DiskSpd 2.3 XML parser now exists in `src/main/benchmarkXml.ts`.
It uses a locked third-party XML parser rather than regex to interpret XML.
The adapter enforces: 1 MiB input bound, 16,000 tag bound, DTD/entity/comment
prohibition, structural validation, one timespan/target/thread, pinned engine
version, expected scratch target pathname, workload size/queue/random/write ratio,
safe numeric bounds, nonzero test duration, internally consistent I/O counts and
latency percentile monotonicity.

Reference fields came from Microsoft's upstream
`XmlResultParser/XmlResultParser.cpp` and `Process-DiskSpd.ps1`:
`Results.TimeSpan.TestTimeSeconds`, `Thread.Target.ReadBytes`,
`Thread.Target.WriteBytes`, `ReadCount`, `WriteCount`,
`Latency.AverageReadMilliseconds`, and the percentile `Latency.Bucket` list.
The test fixtures are **schema-faithful synthetic examples**, not genuine
hardware measurements. MB/s is decimal: bytes / elapsed seconds / 1e6.

A renderer view-model also accepts a future validated `BenchmarkReport`,
but it never presents scores for sample disk identities and no live
benchmark report is connected to the current UI. The worker, owned scratch
file, identity validation, process cancellation, and hardware tests remain
separate next-phase work.

## P2.5A.3 — Native DiskSpd XML smoke in GitHub Actions

A separate Windows Actions workflow downloads the **official Microsoft
DiskSpd 2.3** ZIP and checks the SHA-256 digest reported by GitHub Releases
before extracting or running the binary:

`18609981e8b04b4f645f1fd182ad0541615fe71722de36a9a933c54f747218a3`

The test creates exactly one new small scratch file in GitHub's ephemeral
`RUNNER_TEMP`, runs fixed **100%-read** options (`-w0 -Rxml -L`) against
that file only, parses genuine DiskSpd XML and deletes the same scratch file.
It never requests elevation or touches a raw physical disk. The test must not
report CI runner speed as representative benchmark scores.

This is validation code, **not a production storage worker**. The app still
cannot benchmark actual volumes, and no downloadable DiskSpd binary is
distributed to Partition 11 users.

The v2.3 release archive's actual XML `System.Tool.Version` is
`2.3.0` as specified by Microsoft's upstream
`Common/Version.h` (MAJOR=2, MINOR=3, BUILD=0), distinct from the
GitHub release tag `v2.3`. Both the integration test and synthetic fixtures
pin the exact reported XML version. No range or wildcard bypass is allowed.
