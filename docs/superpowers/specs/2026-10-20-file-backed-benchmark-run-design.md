# P2.5 file-backed benchmark execution — design

Status: Approved by the user on 2026-10-20; implementation and verification are in progress under the approved plan.
Scope: Connect the inert Benchmark screen to a bounded, file-backed DiskSpd run on a freshly revalidated eligible Windows volume. Physical-device acceptance remains the user's task.

## Goal and exclusions

A trusted user can start one allowlisted DiskSpd profile against one new application-owned scratch file on an eligible local volume, inspect validated results, and cancel after the whole child process tree is drained and the exact scratch object is removed.

This measures the mounted filesystem and Windows storage stack; it does not measure an unmounted ext4 partition. No physical disk, partition, or volume-device handle may be opened for writing. No elevation, formatting, repair, mounting, arbitrary target path, shell command, dynamic engine download, remembered consent, or hidden background run is added. Inventory eligibility alone never authorizes a benchmark.

## Trust boundary and run flow

1. The renderer calls typed start/cancel APIs. A start request contains only the current opaque volume-selection token, a fixed profile ID, direction, and request ID. Renderer input never contains a path, device number, identity hash, executable, command switches, arbitrary duration, repetitions, or XML.
2. Electron main validates the trusted top-level frame, consumes the current selection token once, derives the fixed workload and impact estimate, and permits one active run.
3. Main displays a native confirmation dialog bound to the volume and profile. Write direction requires a distinct affirmative choice for each run. Decline creates no file and launches no worker. There is no remembered consent.
4. Main directly starts the fixed packaged Rust agent without a shell, using a versioned, bounded stdin/stdout protocol. It sends a one-run authorization envelope and an ephemeral HMAC key. No raw device identity or filesystem path is sent to the renderer or logs.
5. After consent, the agent performs a fresh complete disk/volume scan and recomputes the keyed identity. It rejects identity drift, stale mapping, ambiguity, PnP change, timeout, malformed input, ineligibility, or insufficient current free space before creating a file.
6. The agent creates one bounded CREATE_NEW scratch file at the selected volume root, verifies the held handle and file ID, then invokes verified DiskSpd with a fixed argument vector and that exact volume-GUID file path.
7. Main accepts only bounded status frames and the final bounded XML. It validates the full response and binds parsed results to the request ID, profile, direction, scratch target, engine version, and exit status before displaying them.
8. On cancel, timeout, window close, or shutdown, the agent terminates the owned Job Object, waits for every contained process to exit, drains bounded stdout/stderr, deletes only the held scratch object, and verifies deletion. Failure to drain or clean is a failure state, not success; another run is blocked until resolved.

The benchmark protocol version is independent of inventory and renderer API versions. It accepts exact allowlisted fields only. Request IDs correlate a run; they are not agent authentication. Per-run HMAC material stays in memory and is discarded. Refresh, denial, cancellation, or completion invalidates the selection token.

## Consent and fixed workload

The native dialog shows the volume label/mount, filesystem, profile, read/write direction, scratch size, maximum duration, write-rate ceiling, and conservative maximum bytes written to the test file. Main computes the estimate from the actual fixed arguments and termination grace; renderer input cannot reduce or change it.

Initial execution is one profile and direction per consent, one thread, one repetition, 1 GiB scratch file, up to 15 seconds warmup and 2–60 seconds measured time. The allowlist is sequential 1 MiB Q8T1/Q1T1 and random 4 KiB Q32T1/Q1T1. UI must not imply one consent covers the whole suite.

Write runs use fixed write-through mode and a 100,000-byte-per-millisecond cap (100 MB/s decimal) for one thread and one target. The maximum-byte disclosure includes warmup, measured duration, and the full termination grace. A single tested conversion function supplies both the actual cap and estimate. No arbitrary rate or repetition is accepted. Read runs use unbuffered mode and disclose zero benchmark-target write bytes; incidental OS metadata writes are outside that estimate.

The selected filesystem must pass all volume-eligibility checks again immediately before scratch creation. Benchmark support may be narrower than inventory support if safe create/share/alignment/delete/recovery behavior has not been proven for a filesystem. Unknown behavior fails closed.

## Scratch ownership and cleanup

The worker records the held file handle, volume serial, 128-bit file ID, canonical volume-GUID path, bounded size, and random basename in memory. It never truncates an existing file, creates a directory, cleans by wildcard, or passes a volume root as DiskSpd target.

Normal completion and cancellation delete through the held handle only after the Job Object is empty, then verify the identity and deletion. Process teardown must not leave an untracked ordinary test file. The implementation plan must prove delete-on-close/share-delete compatibility with the pinned DiskSpd binary on an isolated, non-physical test filesystem. Microsoft documents that deletion occurs after the final handle closes and subsequent opens require FILE_SHARE_DELETE. A filesystem that cannot meet this property is disabled; there is no silent fallback to abandoned files.

For power loss or filesystem failure, an atomic pending-lease record may be required. If used, it stores only exact scratch basename, file ID, volume binding, size, and random lease ID, protected by the current Windows user's data-protection facility. It never stores PnP paths, instance IDs, serials, or raw stable identifiers. Recovery opens only the exact name without following reparse points, verifies the recorded file identity and bounds, and deletes by handle. Mismatch leaves the file untouched and reports cleanup required. Filename-pattern-only cleanup is forbidden.

## Engine, process, result, and packaging

Use only official Microsoft DiskSpd v2.3 AMD64 from the pinned release archive, SHA-256 18609981e8b04b4f645f1fd182ad0541615fe71722de36a9a933c54f747218a3. Before invocation, also pin the extracted executable SHA-256 in the manifest. Use the existing handle-based verifier and reject wrong architecture/path, reparse points, unexpected size/hash, or replacement. Never download at runtime.

Launch only through the existing benchmark-worker Job Object: create the child suspended inside a kill-on-close job before resume. Bound runtime, child count, stdout/stderr, and XML size. Cancellation terminates and drains the whole job. The renderer cannot choose arguments. Do not use DiskSpd create/truncate switches; the lease already created the file. Reject raw target forms, volume roots, UNC paths, alternate streams, extra targets, and shell metacharacters.

Main validates one XML result with the existing strict parser and verifies current run/profile/direction/target/version/duration/counts. DTD/entities, extra targets/spans/threads, inconsistent counts, nonzero exit, stale IDs, or post-cancel output fail closed. Raw XML, logs, local paths, volume identity, and native errors never reach renderer or ordinary logs.

Reports contain measurements only after validation: profile, direction, decimal MB/s, IOPS, available latency percentiles, engine version, filesystem, scratch size, cache policy, duration, and timestamp. They omit stable hashes, device serials, PnP identifiers, and executable paths; demo data stays separate.

The Windows x64 preview packages DiskSpd with its MIT notice and a CycloneDX SBOM component containing version, source, architecture, license, and executable hash. Build verifies archive and executable hashes and tests exactly one expected engine is packaged. Record app/agent/engine/notice/SBOM hashes in the preview manifest. This remains an unsigned test preview, not production-release approval.

## Acceptance and no-go

- Unit tests cover exact protocol, stale/replayed tokens, trusted-frame rejection, HMAC revalidation, consent denial, write-byte arithmetic, binary pin/replacement, scratch handle identity, delete-on-close/share-delete, XML binding, cancellation races, job drain, and cleanup failure.
- Windows CI runs the pinned binary only against an isolated disposable file; verifies real XML, policy arguments, cap configuration, cancel, and exact cleanup. It does not query or benchmark host physical disks.
- Windows UI/package smoke verifies consent before scratch creation, start/cancel, sanitized errors, and packaged resources without a physical-device run.
- Local build verifies packaged engine and hashes; record the final unsigned x64 ZIP and its SHA-256.
- The user performs USB/NVMe/SATA, hotplug, filesystem, BitLocker, and performance acceptance on Windows 11. The implementer runs no physical-device benchmark.
- Any raw device write, elevation, arbitrary path, missing consent, stale/ambiguous selection, changed mapping, unsupported filesystem, unverified hash, escaped child, unbounded output/runtime, uncertain cleanup, invalid XML, or missing license/SBOM keeps benchmark availability disabled. Other write capabilities remain false.

## Sources

- Microsoft DiskSpd command line: https://github.com/microsoft/diskspd/wiki/Command-line-and-parameters (the -g value is a per-thread/per-target byte-per-millisecond throughput cap by default; suffix i means IOPS).
- Microsoft CreateFileW: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew (delete-on-close and FILE_SHARE_DELETE rules).
- Repository context: docs/P2_5_DISK_BENCHMARK.md, third_party/diskspd/manifest.json, crates/benchmark-worker, and the approved volume-eligibility addendum in docs/superpowers/specs/2026-10-19-readonly-disk-preview-design.md.
