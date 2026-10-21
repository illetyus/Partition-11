# P2.5 test-candidate contract addendum

Status: Proposed for review on 2026-10-21; no product behavior changed by this document.

This addendum preserves the approved file-backed benchmark design and ADR-0004. It resolves preparation, timing, transport, and readiness gaps found while planning the remaining work. It does not authorize physical-device acceptance or enable P4–P8 features.

## Intended outcome

Deliver an unsigned Windows x64 package with metadata discovery, read-only ext-image inspection/export, and an explicitly consented, bounded file-backed benchmark. The user performs Windows 11 hardware, hotplug, mixed-DPI, Narrator and high-contrast acceptance. Automated validation uses synthetic images, temporary files and mocked inventory; the real pinned DiskSpd executable runs only in disposable Windows CI storage.

## Decisions proposed for this candidate

### 1. Initialize the benchmark file and disclose preparation writes

Current `ScratchLease::set_file_size` sets EOF and flushes; it does not populate the complete file. Microsoft distinguishes file size from valid data length, so this is insufficient evidence for meaningful reads of initialized test data. This is a correctness concern inferred from the source and API contract, not a measured device-performance defect. See [SetEndOfFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setendoffile).

Proposed behavior: after consent, fresh eligibility validation, and durable `ready`, initialize the exact 1 GiB scratch file with bounded ordinary writes, then flush before DiskSpd starts. Use 1 MiB buffers filled from the existing OS cryptographic random source; never call `SetFileValidData`, request privileges, or bypass the filesystem. Verify exact bytes written, identity, size and cancellation before transitioning durably to `running`. Preparation has a 30-second deadline and the same 100,000,000 logical bytes/second rate ceiling. An I/O that cannot finish within the watchdog budget causes cancellation/termination and recovery-required state; no success is inferred from a termination request.

Both read and write consent disclose the 1 GiB preparation write. The measured read workload writes zero data bytes; the complete read operation does not. Preparation is excluded from throughput/latency measurements. Reject sparse, compressed, encrypted, reparse or otherwise unsupported scratch-file attributes rather than presenting them as ordinary NTFS results. State that results measure this file, filesystem and Windows storage stack, rather than raw media or an unmounted ext partition.

Alternatives: keep an unwritten file (reject: unclear measurement meaning); use privileged valid-data shortcuts (reject: security and disclosure conflict); initialize a private file after consent (recommended).

### 2. Align all deadlines and impact disclosure

Keep the approved workload: four existing profiles, one thread, one target, one repetition, 1 GiB file, 5-second warmup, 15-second measurement, pinned DiskSpd XML version 2.3.0, and `-g100000` for write direction.

Current native process supervision additionally permits 10 seconds for startup/output and 15 seconds for termination. Current consent arithmetic counts only 5 + 15 + 15 seconds. Use one explicit contract shared by Rust, TypeScript and cross-language tests:

| Budget | Proposed value | Failure behavior |
|---|---:|---|
| Fresh metadata scan | 30 s | Reject before file creation; cancel on generation change |
| File preparation | 30 s | Abort preparation, preserve recovery evidence if uncertain |
| DiskSpd including startup/output | 30 s | Terminate Job Object |
| Job termination/drain | 15 s | Retain journal and block next run if not empty |
| Normal cleanup allowance | 10 s | Recovery-required on uncertainty |
| Agent watchdog | 120 s | Exit/termination attempt, no successful result |
| Main hard deadline | 130 s | Kill attempt, quarantine until child close; block another run |

These are application response budgets, not a guarantee that an unresponsive kernel driver can be forced to finish. No timeout permits freeing live native I/O memory or deleting a file before its writer is known stopped.

Impact disclosure includes preparation, the full possible write-active interval, termination allowance and any engine throttle/outstanding-I/O allowance proven by the pinned implementation. The basic rate-times-time component is 4,500,000,000 bytes for 45 seconds, plus 1,073,741,824 preparation bytes; it is not by itself a proven strict maximum. Add a documented conservative allowance backed by the pinned engine's scheduling semantics before using a maximum-byte claim. Do not use the existing 3.5 GB number unchanged. If an enforceable upper-bound claim cannot be established, keep readiness blocked and present a reviewed revised contract; never silently weaken the approved bounded-write requirement. Filesystem metadata and device-internal write amplification are not reported as exactly bounded host data bytes.

### 3. Keep cancellation available and wait for process closure

Use one agent per run. Send a maximum-2-KiB authorization JSON frame followed by a newline, keeping stdin open. Consume only the first frame; the remaining channel accepts only `cancel\n`. EOF, malformed control input and parent disconnect cancel the run. Main never sends HMAC keys through arguments, environment variables or logs. This replaces EOF-delimited request reading for this command only.

Agent final messages have exact fields and matching request ID:

- Completed: `{ protocolVersion: 1, requestId, status: "completed", ownedTargetPath, filesystem: "NTFS", xml }`.
- Cancelled: `{ protocolVersion: 1, requestId, status: "cancelled" }`, only after confirmed cleanup.
- Error: `{ protocolVersion: 1, requestId, status: "error", error }`, where `error` is one of `invalid_request`, `stale_selection`, `worker_failure`, `cleanup_required`.

Final envelopes are emitted only once a syntactically valid request ID is available. Malformed input without a validated request ID exits nonzero without echoing untrusted fields. Raw XML is limited to 1 MiB. The final JSON transport is limited to 8 MiB to allow worst-case JSON escaping of bounded XML; native stderr is limited to 16 KiB. Parse once after `close` with strict keys, UTF-8 and request matching; do not accept multiple final frames or trailing data. Buffer only up to the cap and discard excess after initiating cancellation. Only main sees paths/XML; the renderer receives the sanitized report contract; an authenticated stale selection maps to its existing renderer-facing `stale_selection` status.

Node distinguishes process exit from stdio closure; use `close` as the transport completion event. A kill request does not prove the Rust Job Object drained. See [Node child process events](https://nodejs.org/docs/latest-v24.x/api/child_process.html#event-close) and [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

### 4. Preserve recovery authority on every uncertain path

Disable automatic `ScratchLease::Drop` deletion before preparation or process execution can leave uncertain work. Explicit cleanup requires known stopped preparation I/O and an empty contained Job Object. Panic, watchdog, undrained job and uncertain native errors close owned handles but retain the protected journal for exact recovery. Never interpret Drop or process exit as cleanup proof.

Recovery and execution share a serialized main-process lifecycle. Recovery timeout/output failure cannot release the child slot before `close`; a surviving recovery child keeps readiness blocked. Neither inventory refresh nor a later recovery response may clear a running operation's latch. Only confirmed clean recovery with no active/quarantined child restores eligibility for a new selection.

### 5. Separate automated readiness from hardware acceptance

Availability is a discriminated `preview`/`ready` result computed in main from the reviewed build gate, verified bundled engine, clean recovery, platform and child lifecycle. Renderer input or an environment override cannot turn it on. Fresh eligible selection and per-run consent remain necessary even when availability is ready.

First pass all lifecycle and integration tests with the build gate false. Then a separate reviewed activation commit changes the gate and reruns the complete candidate CI. Deliver only the artifact built from that final checked commit. The user's Windows 11 acceptance remains pending; merge, signing and public release remain excluded.

## References

- [Approved execution design](2026-10-20-file-backed-benchmark-run-design.md).
- [Approved recovery design](2026-10-20-p2-5-scratch-recovery-journal.md) and [ADR-0004](../../decisions/0004-p2-5-scratch-recovery-journal.md).
- [Pinned DiskSpd target-open source](https://github.com/microsoft/diskspd/blob/5e7025bfc9d1364f185d4c30963d7ac79195435e/IORequestGenerator/IORequestGenerator.cpp): read/write sharing, no share-delete. Retain the approved journal alternative.
- [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew): reciprocal sharing checks and delete-on-close restrictions.
- [Electron security](https://www.electronjs.org/docs/latest/tutorial/security): preserve renderer sandbox/isolation and validate IPC sender.
