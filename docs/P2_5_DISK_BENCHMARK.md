# P2.5 — Disk Benchmark: literature review and implementation plan

## Current implementation checkpoint — 2026-10-20

The approved spec now has a strict Rust one-shot request parser and a pure
fresh-target validator. It binds the requested volume/disk/partition ordinals
to the re-scanned eligibility result, keyed identity hash, uniqueness across
all scanned disks, NTFS, and the minimum free-space bound. A Rust/TypeScript
golden HMAC test confirms both sides encode the same authorization message.
The agent does not dispatch this mode or create files yet.

Electron now has exact-shape start/cancel IPC, one-active-run state, one-use
selection consumption, per-run native consent details, cancellation gating,
and a shared fixed impact estimate. The production runtime gate remains
explicitly false, so these APIs return unavailable and the renderer controls
remain disabled. Main tests cover malformed renderer requests, consent denial,
replay, busy/cancel races, and refusing success before cleanup.

Current verification: Rust workspace 180 passed / 4 ignored; main tests 71/71;
strict Clippy, fmt check, full TypeScript/Electron build, and mock-only Windows
UI smoke passed. The pinned DiskSpd delete-on-close test is intentionally
ignored locally and has not run in CI. Therefore the CI compatibility gate,
production scratch ACL/lifecycle, agent dispatch, child-process integration,
validated result path, active UI, and candidate package remain incomplete. No
physical inventory, DiskSpd workload, or volume write ran.

Task 1 local regression coverage — 2026-10-20

The Windows x64 worker suite now covers junction-root rejection, replacement at a captured file path, and the FILE_ATTRIBUTE_REPARSE_POINT file-metadata rule. The suite passed 28 tests with 3 ignored; the full locked workspace also passed agent 122, worker 28, core 20, and ext-reader 16 tests. Workspace fmt, warning-denying Clippy, and locked release build passed.

The explicit process-exit lease cleanup probe produced the expected RED: the current ScratchLease leaves its file after owner-process exit without Drop. It remains ignored pending the Windows CI DiskSpd delete-on-close compatibility gate. The gate has not run; the production lease/runtime remain unchanged and disabled. No DiskSpd workload or physical-device test ran.

A direct file-symlink fixture could not be created here (Win32 error 1314). The local reparse checks use an unprivileged directory junction plus a pure test of the file reparse metadata rule; this does not prove file-symlink handle behavior on this host.

Reviewed: 2026-10-19. P2.5A mock UI, typed disabled IPC, strict XML parser and CI XML integration are implemented; P2.5B.1 owned scratch-file/workload-planner primitives and P2.5B.2 bounded write-rate planning are implemented for tests. PR #51 merged P2.5B.3 protected MBR/GPT partition metadata through worker protocol v2. Local P2.5B.4 adds bounded drive-letter-to-partition mapping, fail-closed volume eligibility, and a process-local HMAC binding for stable disk identities. The unsigned x64 candidate and production scratch-file integration, pinned-engine, consent, Job Object, and user hardware gates are recorded in `docs/PLAN.md`; production benchmark launch remains disabled. No production benchmark worker is connected to the UI, no hardware benchmark is enabled, and no raw writes are allowed. See SESSION_HANDOFF.md.

Current 2026-10-20 status supersedes older dated checkpoint text below: the
official pinned DiskSpd v2.3 AMD64 executable, MIT notice, manifest, and a
component CycloneDX SBOM are included in the unsigned preview package inputs.
The app still cannot start a benchmark. The delete-on-close compatibility test
is prepared for Windows Actions but has not run on this local branch, so the
production lease remains unchanged and the Benchmark Start/Cancel controls
remain disabled.

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
| Microsoft volume handles | https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew | Direct volume access requires administrator privileges; excluded from the non-elevated agent |
| Win32_LogicalDiskToPartition | https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-logicaldisktopartition | Supported WMI association between a logical drive and its partition |
| Win32_DiskPartition | https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-diskpartition | Read-only partition disk index, start offset and size |
| Get-CimAssociatedInstance | https://learn.microsoft.com/en-us/powershell/module/cimcmdlets/get-cimassociatedinstance?view=powershell-5.1 | Local CIM association query used by the bounded preflight |

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

## Initial workload presets (definitions implemented; live execution NOT enabled)

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
- [ ] P2.5B pinned DiskSpd executable and license notices (local manifest/notice added; workflow and production-distribution gates remain open)
- [ ] P2.5B create-new scratch file, volume/ownership verification
- [ ] P2.5B raw targets, symlinks, UNC paths and injections rejected
- [ ] P2.5B write-rate ceiling and consent byte estimate enforced by live worker
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

## P2.5B.1 — Workload allowlist and CI-only scratch lease

The native Electron main source now defines:
- `benchmarkPolicy.ts`: an allowlisted four-profile / two-direction argument
  planner with strict finite durations, bounded repetitions, no command
  shell, no arbitrary user switches and rejection of UNC/raw-device targets.
- `benchmarkScratch.ts`: exclusive `wx+` scratch-file creation in a
  private, uniquely named test directory, limited to 4–64 MiB, aligned,
  with file/directory identity checks and exact-file cleanup.
- `benchmarkPolicy.test.ts`: workload, path injection, ownership,
  invalid-size and replaced-file security regression cases.

**Not enabled in production:** These are CI-grade internal primitives, not
an end-to-end Windows volume authorization mechanism. Windows native
volume/handle identity, system disk exclusion, exclusive writable volume
authorization, full process-tree cancellation via Windows Job Objects,
and a user-facing consent flow remain mandatory P2.5B release gates.
No Electron IPC launch method or real write benchmark was introduced.

## P2.5B.2 — Fixed write-rate ceiling and conservative byte estimate

The workload planner adds DiskSpd's `-g100000` (100,000 bytes/ms) only to
write workloads. The policy fixes one thread and one target, so the
per-thread/per-target rate ceiling is 100 MB/s decimal. Read workloads remain
uncapped so their throughput is not intentionally reduced.

The planner returns a conservative write-byte estimate equal to that rate
multiplied by the full process deadline for every repetition. The deadline
includes warmup, measurement and the existing 15-second termination allowance;
the maximum accepted settings therefore estimate at most 27,000,000,000 bytes
for one profile run (15-second warmup, 60-second measurement, three repeats).
The later consent screen must present this bound before a write run and still
mention flash wear and thermal effects. This estimate is not yet connected to
the UI or a production worker, and it is not proof of any live write behavior.

The official DiskSpd command reference defines `-g` as a per-thread,
per-target bytes-per-millisecond throughput throttle:
https://github.com/microsoft/diskspd/wiki/Command-line-and-parameters

## Local P2.5B.4 — Non-elevated volume eligibility preflight

The agent obtains the OS volume GUID from `GetSystemWindowsDirectoryW`,
enumerates volume metadata with the documented volume-management APIs, and
queries the local `Win32_LogicalDiskToPartition` association through a fixed
PowerShell command. It uses a 3-second CIM operation timeout and kills the
PowerShell process after 5 seconds; output is limited to 512 KiB and 4,096
numeric mapping rows. The mapping is queried twice in a scan. Each extent
must map to exactly one current disk record and one occupied partition range
from the independent Windows disk-layout query. WMI access or parsing failure
never grants eligibility.

Direct `CreateFile` volume handles are not used for extent lookup because
Microsoft documents administrative privileges as a requirement for direct
volume access. This application rejects elevated execution and does not
request UAC. The local WMI query is read-only; if PowerShell/CIM is unavailable,
blocked by local policy, times out, or returns an incomplete mapping, targets
remain ineligible. The renderer receives no volume GUID or WMI path, only a
drive-letter root and bounded metadata. System-volume drive letter aliases
and folder mount aliases are both considered when rejecting ambiguous
mounts.

Stable-identity uniqueness is fail-closed across the full scanned disk list:
if any enumerated disk is unavailable or its stable identity cannot be read,
the candidate volume is `identity_unavailable`, because uniqueness cannot be
proven from a partial set. This local rule is covered by an agent unit test.

This is a **local implementation checkpoint**, not a completed acceptance
gate. Rust policy/decoder tests, Clippy, Windows x64 release compilation,
TypeScript typecheck, main-process tests, production Electron build, and
Windows UI smoke pass. The UI smoke used static data, passed 45 assertions
and captured 11 screenshots; it confirms app launch does not query a physical
disk. The physical Windows 11 inventory test is intentionally left for the
user. Benchmark launch, file creation and writes remain disabled.


## Local P2.5B.5 — Isolated Windows scratch-file lease (2026-10-20)

A Windows-only library crate, crates/benchmark-worker, now contains an internal ScratchLease primitive. Its production entry point accepts only a mounted volume root, avoiding arbitrary ancestor paths that could be redirected between validation and file creation. It atomically creates one cryptographically random CREATE_NEW file directly at that root with a 4 MiB to 1 GiB, 4096-byte-aligned size bound. It captures the returned file handle's identity (volume serial plus 128-bit file ID), obtains the normalized volume-GUID path, rejects reparse points and files with multiple links, and holds the handle without FILE_SHARE_DELETE to block rename/removal while leased. Verification reopens the path and compares it with the captured handle identity. Cleanup marks only the captured file for deletion by handle; no directory is created or removed. If post-CREATE_NEW identity or volume validation fails, the code attempts to delete that exact file handle. The temporary-directory root helper is compiled only for tests.

CREATE_NEW creation and FILE_FLAG_OPEN_REPARSE_POINT semantics follow [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew); unique names use [BCryptGenRandom](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom) with the system-preferred RNG flag.

The Win32 layouts were checked against Microsoft's FILE_STANDARD_INFO and FILE_DISPOSITION_INFO definitions: their BOOLEAN fields are one byte. See [FILE_STANDARD_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_standard_info), [FILE_DISPOSITION_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_disposition_info), [GetFileInformationByHandleEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandleex), [FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info), and [GetFinalPathNameByHandleW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew), and [SetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle).

The crate is not a binary, is not packaged or connected to Electron/agent IPC, and does not launch DiskSpd or access physical disks. Four Windows tests used only a temporary directory and passed; strict workspace Clippy passed. These checks validate the local primitive only. No volume eligibility scan, real-drive test, benchmark, raw disk access, or write workload was run. The prior Electron ZIP predates this crate and does not contain it.

The P2.5 acceptance item for an integrated production scratch-file target remains open. In particular, filesystem ACL/privacy policy, recovery after process crash, DiskSpd pinning and target validation, consent, process-tree cancellation, verified live metrics, CI integration, and the user's physical Windows 11 acceptance remain gates.

## Local P2.5B.6 — DiskSpd CI provenance and license notice (2026-10-20)

The official Microsoft DiskSpd v2.3 release asset is recorded in
third_party/diskspd/manifest.json with its release tag, GitHub-reported
signed commit prefix, amd64 architecture, XML result version 2.3.0, official
asset URL, and pinned SHA-256. The matching MIT notice is preserved beside
the manifest. The Windows read-only CI workflow now reads those values from
the manifest, verifies the ZIP digest before extraction, selects exactly one
amd64 executable, and passes the pinned XML version into its strict parser.

A regression test checks the manifest and MIT notice and guards the current
Electron packaging allowlist against including DiskSpd. This is source-level
evidence only until the updated GitHub Actions workflow passes. The executable
is not checked into the repository, packaged in the app, connected to a
production worker, or run against a user volume. Production redistribution
still requires delivery of the MIT notice and a full release/SBOM review.
## Local P2.5B.7 — Internal bounded Windows process supervisor (2026-10-20)

The Windows-only `partition11-benchmark-worker` crate now contains a crate-private process supervisor for a future file-backed benchmark worker. It creates an anonymous Job Object with kill-on-close and a four-process cap, then uses `PROC_THREAD_ATTRIBUTE_JOB_LIST` to create the child already contained in that job and suspended before resuming it. `STARTUPINFOEX` also carries an explicit allowlist containing only the NUL stdin handle and the child stdout/stderr pipe writers; unrelated parent handles are not inherited. Microsoft's API reference documents this job-list attribute for Windows 10 and newer; process creation fails closed if setup cannot use it. See [UpdateProcThreadAttribute](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute) and [InitializeProcThreadAttributeList](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-initializeprocthreadattributelist).

The parent polls and drains both anonymous pipes while checking cancellation, the validated warmup-plus-measurement deadline, and the Job Object active-process count. Captured stdout is limited to 1 MiB and stderr to 16 KiB. Timeout, cancellation, output overflow, and pipe/query failures request termination of the entire job and wait up to 15 seconds for it to empty. Failure to verify an empty job is returned as `Undrained`, including an accounting-query error during drain; a future caller must keep the scratch lease and must not claim cleanup is safe on that result. A normal return waits for descendants as well as the primary process and captures the bounded output.

Windows worker verification on this host: `cargo fmt --all -- --check`; `cargo test --locked -p partition11-benchmark-worker` (15 passed, one ignored helper); `cargo clippy --locked -p partition11-benchmark-worker --all-targets -- -D warnings`; and `cargo build --locked --workspace --release`. Tests use local command fixtures and an ignored helper that starts a short PowerShell sleeper. No DiskSpd executable was launched, no physical volume was queried or written, and no live inventory or hardware acceptance was performed.

This remains an internal primitive, not a production benchmark worker. It has no production command constructor, no IPC or Electron connection, no package integration, no consent flow, and no volume authorization. DiskSpd remains CI-only and excluded from the application package. User-run Windows 11 hardware acceptance remains a later, separate gate.

## Local P2.5B - Write-impact disclosure (2026-10-20)

The preview displays a bounded maximum byte/time estimate for the write profile as repeat count changes. A shared helper and the main-process workload planner use the same 100 MB/s throttle and include the per-run 15-second termination allowance. The only preview test-file size is 1 GiB, matching the native ScratchLease maximum. Unit tests cover 1 and 3 repetitions; the mock-IPC Electron UI smoke verifies the displayed 3.5 GB / 35 second and 10.5 GB / 105 second upper bounds.

This is not the required separate per-run consent flow. Start and Cancel remain disabled, and the estimate does not prove actual write behavior. Production worker integration, target/identity binding, ACL and crash recovery, validated live XML metrics, CI and user hardware acceptance remain open. Local UI behavior passed; screenshot capture was skipped because this Windows host's Electron compositor returned `UnknownVizError`.

## Current local checkpoint — P2.5 safety-gate preparation (2026-10-20)

The official Microsoft DiskSpd v2.3 AMD64 executable is pinned by its upstream
archive digest and executable SHA-256, configured for inclusion only in the
next unsigned preview, and accompanied by the MIT notice and a component
CycloneDX 1.5 record. The
Windows DiskSpd workflow now checks both hashes and schedules an ignored Rust
test that runs the exact pinned engine read-only against a temporary file
opened with delete-on-close flags inside the existing kill-on-close Job Object.
This compatibility workflow is **not yet verified** because it has not run on
the local branch. No DiskSpd workload was run on this computer.

Fresh volume records now retain the validated volume-GUID root internally for
later scratch-lease creation. The one-run target validator rejects a missing or
malformed root and returns it as `volume_guid_root`; the public inventory
response does not expose it. The Windows agent's complete unit suite passed 122
tests with two physical-device tests ignored; focused benchmark-protocol tests
passed 8/8, inventory-serialization tests 6/6, and agent Clippy/rustfmt passed.
This validates only in-memory path binding and does not connect or launch a
worker.

Main-process volume selection tokens now expire after 60 seconds and are
consumed once; only the main-process service receives a copy of the keyed
identity secret. The approved contract now permits one profile run per
consent; the repetition selector was removed. Fresh verification passed:
`npm run test:main` 76/76, `npm run build`, and the capture-disabled Windows
Electron mock-IPC smoke. Locked workspace Rust tests passed: agent 122 (two
physical-device tests ignored), benchmark-worker 24 (process-fixture and CI
DiskSpd tests ignored), core 20, and ext-reader 16. Workspace rustfmt,
warnings-as-errors Clippy, and release build passed. The ordinary local
screenshot path previously returned `UnknownVizError`; no screenshot evidence
is claimed.

Production worker launch, consent-bound execution, fresh agent dispatch,
delete-on-close production lifecycle, verified live metrics, and Windows
Actions acceptance remain open. No physical-volume inventory, benchmark,
format, or write was performed. P2.5 and the user's Windows 11 acceptance are
not complete. `npm audit --omit=dev --audit-level=moderate` could not refresh
because this environment could not resolve `registry.npmjs.org`; dependency
advisory status for the current lockfile is unverified.

## 2026-10-20 — Main-process result validation checkpoint

The Electron main-process controller now treats worker output as untrusted. It
requires the exact report/result fields, pinned DiskSpd XML version, fixed 1 GiB
file and 5/15-second timing, matching NTFS volume identity, direction-specific
cache policy, the requested single profile/direction, canonical UTC timestamp,
and finite positive measurements with ordered percentiles. Unexpected fields,
malformed results, and uncertain post-launch failures fail closed and latch
`cleanup_required` for later starts.

The strict XML parser also binds `TestTimeSeconds` to the trusted requested
measurement duration within a 10% tolerance (minimum 250 ms); it rejects an
invalid/unpinned parser context and materially short or long runs before
calculating throughput and IOPS.

Main validates the worker's keyed volume identity but does not return it to the
renderer. The renderer-facing `targetVolumeId` is replaced with the existing
one-use selection token. A shared `2.3.0` engine-version constant is checked
against the bundled manifest. Focused controller/XML/manifest tests passed
30/30; the full main suite passed 76/76; the TypeScript/Electron production build
passed; and the capture-disabled mock-only Windows UI smoke passed 42
assertions. The normal screenshot path returned local compositor
`UnknownVizError`, so no screenshot evidence is claimed. Runtime is still disabled and the agent is
not connected. Fresh Windows Rust checks passed: benchmark-worker 24 (2 CI or
process-fixture tests ignored), core 20, ext-reader 16, workspace fmt,
warning-denying Clippy, and locked workspace release build. Agent tests were
not run as a full suite because they include live-device probes; filtered pure
agent tests passed (benchmark protocol 7, volume eligibility 16, storage
identity 7). The CI-only DiskSpd/delete-on-close compatibility job, production
lease/dispatch/cancellation integration, packaging acceptance, and user-run
physical Windows 11 tests remain open. No DiskSpd process or physical device
was exercised.
