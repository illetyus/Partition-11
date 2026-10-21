# P3-I1 / P3-I2 — read-only ext image inspection

The Windows preview uses the pinned rust-fs-ext4 v0.8.0 through crates/ext-reader and a one-shot Windows agent image worker. It opens one validated local regular image read-only, rejects Windows device/network namespaces and final reparse points, requires ext4 EXTENTS, and rejects unsupported feature flags before directory access. Upstream mutation APIs are not exposed.

The renderer receives only sanitized display text and opaque session, directory, page, and file tokens. Host image paths and raw ext4 name bytes stay in the main/agent boundary. Directory pages are bounded; symlinks are never followed or exported. Listing and export run in a disposable worker with bounded deadlines. Export creates a unique sibling staging file with create-new semantics, syncs it, and commits with a no-replace move; worker failures clean only the staging path it owns. Main-process cleanup runs after the worker exits, including timeout handling. Recovery after an abrupt host-process crash remains an open policy question.

Limits are 64 TiB per image, 4,096 bytes per internal path, 128 KiB and 16,384 entries per directory, 4 GiB per export, and 64 KiB read chunks. Listing has a 20-second deadline, export a 10-minute deadline, sessions expire after 15 minutes, and only one image worker may run at a time.

Independent fixtures come from e2fsprogs 1.47.3. The clean image SHA-256 is 8ace927a36d0cfff8eb07108ceb7a8e67f50e7ff0041ccfbd4d7956338da4a3d; the recovery-pending image SHA-256 is 72d41cd51a078fa906f10dd9bbd4b9d1c841b55db4357281b7d27164ba460db9. The oracle test verifies the known directory tree and file contents and confirms the source image is unchanged. Dirty/recovery-pending and corrupt-superblock tests also verify unchanged inputs. A deterministic 128-case metadata mutation sweep checks for panics and source mutation.

The ext-reader suite has 16 tests: 5 unit tests, 3 dirty/corrupt/mutation tests, 1 independent e2fsprogs oracle test, and 7 read-only image integration tests. Agent image-export tests cover fixture export through the staging transaction, refusal to overwrite, and preservation of a foreign staging-path collision. Main-process tests cover opaque tokens, deadlines, timeout cleanup, and staging collisions.

A separate cargo-fuzz target drives open, bounded listing, one nested lookup, and small file reads using only temporary image files. The earlier Windows x64 nightly smoke used both e2fsprogs fixtures, a 16 MiB input cap, and a 30-second limit: 62 executions completed in 31 seconds, coverage reached 1,205 features, and no crash was found. A later local run used copies of the two fixtures in an isolated `D:\Android\Temp` corpus and completed 185 seconds, 371 executions, 1,226 coverage counters, 1,897 features and 619 MiB peak RSS with no crash. These are short regression runs, not a long fuzz campaign. The Ubuntu CI fuzz job and other workflow edits have not run on GitHub because this branch has not been pushed.

Local verification on 2026-10-20: workspace fmt, warning-denying Clippy, and locked tests passed (agent 113 passed / 2 hardware tests ignored; benchmark-worker 15 passed / 1 process-tree fixture ignored; core 20 passed; ext-reader 16 passed). npm run build passed, main-process tests passed 60/60, and the mocked Windows Electron UI smoke exited 0 with screenshot capture skipped. Fresh RustSec audit loaded 1,338 advisories and found no issue in 31 locked dependencies. Fresh npm audit found zero production findings and only the documented eight Moderate dev-only `sprintf-js` records in the full graph, with no High/Critical findings.

Unsigned x64 preview archive: release/Partition-11-Windows-x64-p3-final-20261020.zip, 163,956,723 bytes, SHA-256 53431df71ad04c4d8f7822bb761139afc80ab827bab83f3f28f9706479a4b699. The archive has 74 entries and every entry decompressed successfully. It contains the Windows agent and validated target-specific CycloneDX 1.5 agent SBOM. Packaged EXE SHA-256: 7046c184b013de855e171d1bfdf07412d1aa897b087f0078f543a2cdee249fe5; app.asar: 8294587098613daed9f173c6a8d586f39360cd5795f07c7aeede7c5ecfb9e183; agent: 66d5ee43450c6b1b3b03ab640c8ecd6bb58c09704161795d634731c00eb32eb6; SBOM: 60d689a058293223f7c5dc9f3e96bf1672484f6349cafddebb8bb484157415f4. The packaged agent reports image and physical-disk inspection enabled, with both image and physical-disk writes disabled. The executable is unsigned.

No physical disk inventory, benchmark, or write was run. The user remains responsible for physical Windows 11 acceptance. Remaining pre-release work includes GitHub workflow verification, a longer fuzz campaign, abrupt-host-crash staging-file policy, and the P2.5 worker/consent integration. Formatting, repair, resize, and mount remain disabled.

## Additional local fuzz campaign — 2026-10-20

The `ext_image` target completed a 601-second Windows x64 run with nightly
Rust 2026-10-17 and the installed MSVC 14.50 toolchain selected through
`VsDevCmd.bat` for that process only. Command: `cargo +nightly-2026-10-17 fuzz
run ext_image <isolated-temp-corpus> -- -max_total_time=600 -timeout=5
-max_len=16777216 -print_final_stats=1`. Result: exit code 0, 10,091 inputs,
1,306 coverage counters, 3,016 features, 138 new units, and 1,034 MiB peak RSS;
no crash or timeout was reported.

The corpus started with hash-verified copies of the clean fixture
(`8ace927a36d0cfff8eb07108ceb7a8e67f50e7ff0041ccfbd4d7956338da4a3d`) and
recovery-pending fixture
(`72d41cd51a078fa906f10dd9bbd4b9d1c841b55db4357281b7d27164ba460db9`). A
small temporary command launcher was inadvertently also present as a third
arbitrary-byte input; it was removed after the run. Both source fixture hashes
were rechecked and unchanged, and the repository corpus remained untouched.
Generated fuzz inputs were kept in a unique temporary directory and removed
after recording the result. The first build attempt lacked `cl.exe`; selecting
MSVC 14.50 for the retry process fixed it without changing persistent system
configuration.

This extends local parser robustness evidence but does not substitute for the
unrun Ubuntu CI fuzz job, long-term fuzzing, abrupt-host-crash recovery policy,
or Windows 11 user acceptance. No physical disk or volume was enumerated,
opened, or modified.
## Actual Windows agent fixture check — 2026-10-20

The built `target/release/partition11-agent.exe` was exercised through its real
`image-worker` stdin/stdout protocol against the independent clean ext4 fixture.
It listed the five root entries including `known.txt`, exported its 36 expected
bytes into a unique temporary directory, committed the destination, and left no
staging file. A second export attempt to an existing destination was rejected;
the existing bytes were preserved and no stage file was created. The source
fixture SHA-256 matched before and after. The temporary output directory was
removed. This verifies the actual Windows agent binary on a synthetic image,
not the Electron renderer/main integration or physical storage.
## Electron main-to-agent fixture check — 2026-10-20

The compiled `ReadOnlyImageInspectionService` was run under pinned Node 24.21.0
with its default worker-spawn path and the actual Windows x64 agent. It listed
the independent fixture, exported the expected 36 bytes, rejected an existing
destination without changing it, left no `.partial` file, and preserved the
source image hash. The harness and outputs were confined to a unique temporary
directory and removed. This exercises the compiled main service to agent
boundary; it does not simulate Electron IPC/rendering.

### Crash-cleanup API constraint

The current stage is created with `CREATE_NEW` and removed on ordinary worker
failure, but an abrupt host crash can leave it behind. Microsoft documents that
`FILE_FLAG_DELETE_ON_CLOSE` deletes after all handles close, while
`FILE_DISPOSITION_INFO.DeleteFile` has no effect for a handle opened with that
flag. Therefore the simple proposal “create delete-on-close, then clear the
flag before successful commit” is not supported by this API contract. See
[CreateFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea#file_flag_delete_on_close)
and [FILE_DISPOSITION_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_disposition_info).
No cleanup behavior was changed; the durable crash-recovery policy remains
pending design approval.