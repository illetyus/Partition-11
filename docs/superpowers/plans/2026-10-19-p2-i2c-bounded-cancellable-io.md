# P2-I2c-7 Bounded Cancellable Metadata I/O Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Windows metadata IOCTL cancellable and bounded, and isolate a non-cooperative storage driver inside a disposable worker process so the parent agent cannot hang indefinitely.

**Architecture:** Keep SetupAPI enumeration, scan-generation policy, and final inventory validation in the parent agent. Move the fixed metadata query sequence into a one-shot child worker that owns the device HANDLE, per-HANDLE PnP registration, `OVERLAPPED` request memory, and I/O buffers. The worker uses overlapped `DeviceIoControl` with a 5-second budget recorded before request submission and `CancelIoEx`; both parent and worker enforce a 20-second per-disk hard deadline. The parent discards every stale, malformed, cancelled, crashed, or killed result.

**Tech Stack:** Rust 1.99.1, direct Win32 FFI (`CreateFileW`, `DeviceIoControl`, `CancelIoEx`, `GetOverlappedResultEx`, `CreateEventW`, `CloseHandle`), SetupAPI/cfgmgr32, std::process anonymous pipes, GitHub Actions Windows/Ubuntu CI.

**Spec:** `docs/superpowers/specs/2026-10-19-p2-i2c-bounded-cancellable-io-design.md`

## Global Constraints

- Per metadata IOCTL completion budget is exactly **5 seconds**.
- Cancellation completion grace period is exactly **1 second**.
- Per-disk disposable worker hard deadline is exactly **20 seconds**.
- The worker arms its own 20-second termination timer before reading input or opening a device, so it cannot outlive a crashed parent indefinitely.
- Only the existing four metadata IOCTLs are permitted: `IOCTL_STORAGE_GET_DEVICE_NUMBER`, `IOCTL_DISK_GET_LENGTH_INFO`, `IOCTL_STORAGE_QUERY_PROPERTY`, `IOCTL_DISK_GET_DRIVE_LAYOUT_EX`.
- No raw-sector `ReadFile`, `WriteFile`, write/format IOCTL, mount, repair, resize, or elevation may be introduced.
- No renderer-controlled path or IOCTL may enter the worker protocol.
- The child must re-enumerate `GUID_DEVINTERFACE_DISK` and require an exact current path + instance-ID pair before opening a device.
- PnP path, instance ID, and temporary disk number remain transient observations, not durable hardware identity.
- Any timeout, cancellation, PnP invalidation, malformed worker response, abnormal exit, or hard kill makes that disk result unavailable.
- `CM_Unregister_Notification` stays outside notification callbacks.
- All Electron physical-disk and write capability flags remain false.
- Production code follows TDD: each new behavior gets a failing test before implementation.
- A code PR may merge only after the existing five CI jobs are green; fresh post-merge `main` must also be five-for-five green.

## Execution checkpoint

The implementation and review fixes are merged in [PR #46](https://github.com/illetyus/Partition-11/pull/46) as `a9cb53344ff560082096d728eb453af0532a7415`. The first review found that the request deadline started after `DeviceIoControl` submission and that a stuck worker relied only on the parent watchdog; both gaps were fixed, and the follow-up review found no actionable issues. Local verification passed: **88/88 Rust tests**, Clippy, formatting, Windows release build, Windows SDK C ABI check, Electron build, and **31/31 main-process tests**. The main-process tests require repo-local `TEMP`/`TMP` because the default AppContainer temp directory rejects the test's rename with `EPERM`. PR #46 passed 5/5 required checks; fresh post-merge `main` also passed 5/5: [CI](https://github.com/illetyus/Partition-11/actions/runs/39195989162), [Windows UI](https://github.com/illetyus/Partition-11/actions/runs/39195989210), [DiskSpd](https://github.com/illetyus/Partition-11/actions/runs/39195989174). Issue #31 remains open for durable hardware identity, physical Windows 11 acceptance, and authenticated read-only IPC/UI; these gates are intentionally outside this implementation plan.

## Review Focus

- **PnP during pending IOCTL:** callback must request cancellation without blocking; the worker must cancel the exact active request and must not start a later IOCTL.
- **Cancellation race:** `CancelIoEx == ERROR_NOT_FOUND` or normal completion racing cancellation must resolve from final overlapped completion state, never from assumptions.
- **Driver ignores cancellation:** request memory must remain alive; the disposable worker must never report success after the 1-second grace, and the parent must kill it by 20 seconds.
- **Malformed worker input/output:** oversized lengths, bad magic/version, trailing bytes, duplicate messages, or more than 128 partitions must fail closed without opening an arbitrary path.
- **Fast unplug/replug:** even a syntactically valid worker reply must be discarded when the parent generation/inventory snapshot no longer matches.

---

## File Structure

**Create**
- `crates/windows-agent/src/metadata_worker_protocol.rs` — pure bounded one-shot wire codec; no Win32 calls.
- `crates/windows-agent/src/overlapped_ioctl.rs` — Windows-only overlapped metadata IOCTL state machine and native FFI backend.
- `crates/windows-agent/src/metadata_query.rs` — Windows-only fixed metadata query/parsing engine used only inside the worker.
- `crates/windows-agent/src/metadata_worker.rs` — hidden worker entry point, parent process launcher, watchdog, pipe I/O, and current-record revalidation.

**Modify**
- `crates/windows-agent/src/pnp_notifications.rs` — expose nonblocking active-I/O cancellation state to the worker.
- `crates/windows-agent/src/disk_metadata.rs` — retain parent scan policy/types; replace in-parent device IOCTLs with one worker invocation per fresh record.
- `crates/windows-agent/src/main.rs` — register internal `__metadata-worker` mode without advertising it as a public diagnostic mode.
- `tests/windows/drive_layout_abi.c` — independently verify `OVERLAPPED` layout and Win32 constants used by the new FFI.
- `docs/P2_I2C_HANDLE_REMOVAL.md` — point to the new cancellation layer once code lands.
- `AGENTS.md`, `docs/SESSION_HANDOFF.md`, `docs/PLAN.md` — final checkpoint only after code/main verification.

No new Rust dependency is required. The worker protocol is a small bounded binary format implemented with the standard library.

---

### Task 1: Add the bounded one-shot worker protocol

**Files:**
- Create: `crates/windows-agent/src/metadata_worker_protocol.rs`
- Modify: `crates/windows-agent/src/main.rs`

**Interfaces:**
- Produces:
  - `pub(crate) const MAX_WIRE_BYTES: usize = 65_536`
  - `pub(crate) struct WorkerRequest { pub path: Vec<u16>, pub instance_id: Vec<u16> }`
  - `pub(crate) enum WorkerReply { Ready(WireMetadata), Unavailable(WireError) }`
  - `pub(crate) struct WireMetadata { ... }` mirroring `DiskMetadata` without OS handles
  - `pub(crate) enum WireError` with explicit variants for access denied, removed, unsupported, malformed, identity changed, layout too large, timeout, cancelled, worker failure, and `Win32(u32)`
  - `encode_request(&WorkerRequest) -> Result<Vec<u8>, ProtocolError>`
  - `decode_request(&[u8]) -> Result<WorkerRequest, ProtocolError>`
  - `encode_reply(&WorkerReply) -> Result<Vec<u8>, ProtocolError>`
  - `decode_reply(&[u8]) -> Result<WorkerReply, ProtocolError>`
- Consumes: `partition11_core::windows_layout::{DriveLayout, LayoutStyle, PartitionRange, MAX_ENTRIES}`.

- [ ] **Step 1: Write failing protocol tests**

Add tests named:
- `request_round_trip_preserves_utf16_path_and_instance`
- `request_rejects_bad_magic_version_trailing_bytes_and_oversize`
- `reply_round_trip_preserves_all_metadata_field_results`
- `reply_rejects_more_than_128_partitions_and_truncation`

Use fixed little-endian magic/version framing. Request path maximum is 16,384 UTF-16 units; instance ID maximum is 2,047 UTF-16 units. Entire frame maximum is 65,536 bytes.

- [ ] **Step 2: Run tests and verify RED**

Run on the branch:
`cargo test --locked -p partition11-agent metadata_worker_protocol --all-targets`

Expected: FAIL because `metadata_worker_protocol` and codec types/functions do not yet exist.

- [ ] **Step 3: Implement the minimal pure codec**

Declare the module from `main.rs` with `#[cfg(any(windows, test))]` so pure codec tests run on Ubuntu test targets too. Reject unknown tags, arithmetic overflow, embedded NULs in path/instance, trailing bytes, oversized frames, and duplicate/unconsumed payload.

Do not add serde or any third-party protocol dependency.

- [ ] **Step 4: Verify GREEN and full Rust suite**

Run:
- `cargo test --locked -p partition11-agent metadata_worker_protocol --all-targets`
- `cargo test --locked --workspace --all-targets`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

Expected: all pass.

- [ ] **Step 5: Commit**

`git commit -am "feat(p2-i2c): add bounded metadata worker protocol"`

---

### Task 2: Add the overlapped IOCTL state machine and Windows ABI oracle

**Files:**
- Create: `crates/windows-agent/src/overlapped_ioctl.rs`
- Modify: `crates/windows-agent/src/main.rs`
- Modify: `tests/windows/drive_layout_abi.c`

**Interfaces:**
- Produces:
  - `pub(crate) const IOCTL_TIMEOUT_MS: u32 = 5_000`
  - `pub(crate) const CANCEL_GRACE_MS: u32 = 1_000`
  - `pub(crate) const WAIT_SLICE_MS: u32 = 50`
  - `pub(crate) enum OverlappedIoError { TimedOut, Cancelled, MalformedResponse, Win32(u32) }`
  - `pub(crate) fn run_worker_ioctl(raw_handle: usize, code: u32, input: &[u8], max_output: usize, cancellation_requested: impl Fn() -> bool) -> Result<Vec<u8>, OverlappedIoError>`
  - private coordinator state `IoDecision::StuckAfterCancel` used by deterministic tests; the native wrapper must not return from this state while kernel I/O can still reference request memory.
- The function is worker-only; it must never be called from the long-lived parent process.

- [ ] **Step 1: Write failing deterministic state-machine tests**

Use an injected private backend inside the module, not real disk I/O. Add:
- `immediate_completion_returns_bounded_bytes`
- `pending_completion_finishes_before_deadline`
- `five_second_deadline_requests_exact_cancel`
- `late_immediate_completion_expires_the_request_budget`
- `pending_issue_that_uses_budget_is_cancelled_immediately`
- `pnp_cancel_request_is_seen_within_one_wait_slice`
- `cancelled_request_reports_operation_aborted`
- `completion_can_win_cancel_race`
- `cancel_not_found_resolves_final_completion_state`
- `cancel_grace_expiry_is_never_reported_as_success`
- `output_larger_than_bound_is_rejected`

The fake backend must prove the state machine does not issue another request after cancellation/timeout.

- [ ] **Step 2: Run Windows Rust tests and verify RED**

Push only the tests first and let the Windows Rust CI job fail for the missing overlapped implementation. Record the failing job URL in the PR.

Expected failure: missing `run_worker_ioctl`/state-machine symbols, not an unrelated compiler error.

- [ ] **Step 3: Implement native overlapped FFI**

Open request events as **manual-reset** events. Zero-initialize one `OVERLAPPED` per request and store the event in `hEvent`. Treat immediate `DeviceIoControl == TRUE` as terminal success; treat `FALSE + ERROR_IO_PENDING` as pending; all other errors are terminal.

While pending, use `GetOverlappedResultEx` in at most 50 ms slices so PnP cancellation state is observed without blocking a notification callback for the full five seconds.

At timeout or cancellation:
1. call `CancelIoEx(handle, &overlapped)` for the exact request;
2. resolve `ERROR_NOT_FOUND` by reading final overlapped state;
3. give completion exactly one additional second;
4. after grace expiry, mark `StuckAfterCancel`; never accept later success as metadata.

If the driver still owns the request after grace, the native wrapper enters a non-returning `hold_pending_request_until_process_exit(active_request) -> !` path. That path parks/sleeps while retaining the event, `OVERLAPPED`, input and output buffers on the worker stack/heap. It never reports metadata and never drops/reuses those resources; the parent 20-second watchdog ultimately terminates the disposable process. Unit tests exercise the private coordinator's `StuckAfterCancel` decision without entering the non-returning native hold.

- [ ] **Step 4: Extend independent Windows SDK C assertions**

Add static assertions for:
- `sizeof(OVERLAPPED)` and `offsetof(OVERLAPPED, hEvent)` on x64;
- `FILE_FLAG_OVERLAPPED == 0x40000000`;
- `ERROR_IO_PENDING == 997`;
- `ERROR_IO_INCOMPLETE == 996`;
- `ERROR_OPERATION_ABORTED == 995`;
- `ERROR_NOT_FOUND == 1168`;
- `WAIT_TIMEOUT == 258`.

Do not replace the C oracle with Rust constants.

- [ ] **Step 5: Verify GREEN**

Run Windows CI-equivalent commands:
- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace --all-targets`
- `./scripts/check-windows-sdk-abi.ps1`

Expected: all pass.

- [ ] **Step 6: Commit**

`git commit -am "feat(p2-i2c): add bounded overlapped metadata ioctl"`

---

### Task 3: Connect PnP notifications to nonblocking active-I/O cancellation

**Files:**
- Modify: `crates/windows-agent/src/pnp_notifications.rs`
- Test: inline module tests in the same file

**Interfaces:**
- Extend `DeviceHandleState` with an atomic active-I/O cancellation flag.
- Produce:
  - `pub(crate) fn request_io_cancel(&self)`
  - `pub(crate) fn io_cancel_requested(&self) -> bool`
- `DeviceIoLease` continues to keep the raw HANDLE from closing underneath active I/O.

- [ ] **Step 1: Write failing callback/state tests**

Add:
- `busy_query_remove_sets_io_cancel_before_returning_veto`
- `remove_pending_sets_io_cancel_without_blocking_callback`
- `remove_complete_sets_io_cancel_without_blocking_callback`
- `query_remove_failed_does_not_clear_cancelled_old_scan`
- `cancelled_handle_refuses_a_new_io_lease`

Assert that callback code performs no wait and no `CancelIoEx`; it only invalidates generation, flips atomic cancellation state, and performs the existing idle-close behavior.

- [ ] **Step 2: Verify RED**

Run on Windows:
`cargo test --locked -p partition11-agent pnp_notifications::tests --all-targets`

Expected: FAIL because active-I/O cancellation state is absent.

- [ ] **Step 3: Implement the minimal cancellation state**

Set the flag before returning from query-remove/pending/complete callbacks. Keep `CM_Unregister_Notification` outside callbacks. Do not clear the old scan after `DEVICEQUERYREMOVEFAILED`; a new worker gets new state.

- [ ] **Step 4: Verify GREEN/full Rust suite**

Run the PnP tests, workspace tests, clippy, and rustfmt.

- [ ] **Step 5: Commit**

`git commit -am "feat(p2-i2c): signal pending metadata io on pnp removal"`

---

### Task 4: Move the fixed metadata query engine into a worker-only module

**Files:**
- Create: `crates/windows-agent/src/metadata_query.rs`
- Modify: `crates/windows-agent/src/disk_metadata.rs`
- Modify: `crates/windows-agent/src/main.rs`

**Interfaces:**
- Move/produce:
  - `pub(crate) fn query_worker_record(record: &DiskInterface, generation: &ScanGeneration) -> ProbeOutcome`
- `metadata_query.rs` owns:
  - non-elevated check used by the worker;
  - `CreateFileW` with `FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED`;
  - `RegisteredDeviceHandle`;
  - fixed IOCTL constants;
  - device-number/capacity/sector/bus/layout parsing and bounded layout retry;
  - calls to `run_worker_ioctl`.
- `disk_metadata.rs` keeps public internal result types and parent scan orchestration; it stops issuing `DeviceIoControl` directly.

- [ ] **Step 1: Write failing behavior tests**

Add tests proving:
- `worker_open_uses_overlapped_flag_constant`;
- only the four fixed IOCTL codes can enter the query function;
- PnP cancellation maps to `MetadataError::Cancelled`;
- 5-second timeout maps to `MetadataError::TimedOut`;
- bounded drive-layout retry stops after cancellation and does not issue the next retry.

Add `TimedOut`, `Cancelled`, and `WorkerFailed` to `MetadataError` before implementing mappings only after RED is observed.

- [ ] **Step 2: Verify RED on Windows CI**

Expected: missing worker query module/error variants.

- [ ] **Step 3: Refactor the existing query/parsers without changing successful metadata semantics**

Move code rather than duplicate it. Preserve:
- capacity/sector/bus/layout optional-result behavior;
- same-handle device-number before/after validation;
- max descriptor/layout bounds;
- non-elevated requirement;
- no raw read/write APIs.

The query function must pass `|| state.io_cancel_requested()` to `run_worker_ioctl`.

- [ ] **Step 4: Verify GREEN**

Run full Rust tests/clippy/fmt and independent ABI oracle on Windows.

- [ ] **Step 5: Commit**

`git commit -am "refactor(p2-i2c): isolate metadata query engine for worker"`

---

### Task 5: Implement the one-shot worker entry point with fresh target revalidation

**Files:**
- Create: `crates/windows-agent/src/metadata_worker.rs`
- Modify: `crates/windows-agent/src/main.rs`
- Modify: `crates/windows-agent/src/disk_metadata.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn run_stdio_worker() -> i32`
  - `pub(crate) fn validate_worker_target(request: &WorkerRequest, current: &[DiskInterface]) -> Result<DiskInterface, WorkerError>`
- Internal CLI token: `__metadata-worker`; do not mention it in normal help/error text.

- [ ] **Step 1: Write failing worker-validation tests**

Add:
- `worker_accepts_exactly_one_fresh_path_instance_pair`
- `worker_rejects_unknown_path_mismatched_instance_and_duplicates`
- `worker_rejects_elevated_execution_before_device_open`
- `worker_reads_exactly_one_bounded_request`
- `worker_rejects_malformed_or_extra_request_bytes`
- `worker_never_accepts_an_ioctl_code_from_protocol`

The request contains only path + instance ID. IOCTL selection remains compiled into `metadata_query.rs`.

- [ ] **Step 2: Verify RED**

Run Windows agent tests. Expected: worker entry/validation functions absent.

- [ ] **Step 3: Implement one-shot worker**

Sequence:
1. reject elevated process;
2. read at most `MAX_WIRE_BYTES` from stdin;
3. decode exactly one request;
4. fresh `enumerate_disk_records()`;
5. require exactly one exact path + instance-ID match;
6. create a fresh worker `ScanGeneration`;
7. execute `query_worker_record`;
8. encode one bounded reply to stdout;
9. flush and exit.

Before processing input, the worker arms an independent 20-second termination timer. If the overlapped coordinator reaches its stuck-after-grace state, `run_worker_ioctl` does not return to this layer. It retains the active request until the worker self-terminates; the parent watchdog provides a second hard deadline and discards all output.

- [ ] **Step 4: Verify GREEN**

Run worker/protocol tests plus full Windows Rust suite.

- [ ] **Step 5: Commit**

`git commit -am "feat(p2-i2c): add one-shot metadata worker mode"`

---

### Task 6: Add the parent worker watchdog and replace in-process metadata I/O

**Files:**
- Modify: `crates/windows-agent/src/metadata_worker.rs`
- Modify: `crates/windows-agent/src/disk_metadata.rs`

**Interfaces:**
- Produces:
  - `pub(crate) const WORKER_DEADLINE: Duration = Duration::from_secs(20)`
  - `pub(crate) fn probe_via_worker(record: &DiskInterface, generation: &ScanGeneration) -> ProbeOutcome`
- Private process abstraction used by tests:
  - `trait WorkerChild { fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>; fn kill(&mut self) -> io::Result<()>; }`

- [ ] **Step 1: Write failing watchdog tests**

Add fake-process tests:
- `worker_success_before_deadline_decodes_reply`
- `worker_abnormal_exit_is_rejected`
- `worker_malformed_reply_is_rejected`
- `worker_deadline_kills_and_discards_valid_output`
- `generation_change_while_worker_runs_discards_valid_reply`
- `worker_result_is_rejected_when_post_scan_inventory_changes`

The timeout test uses an injected clock/poller; do not make CI sleep for 20 real seconds.

- [ ] **Step 2: Verify RED**

Run Windows agent tests. Expected: watchdog/launcher absent.

- [ ] **Step 3: Implement parent process launch**

Use `std::env::current_exe()` + direct `Command`; never invoke a shell. Pass only the internal mode token on argv. Send the selected path/instance frame through piped stdin, then close stdin.

Read stdout concurrently into a bounded 65,536-byte collector so pipe capacity cannot deadlock the child. Do not log or expose path/instance data.

Poll `try_wait` until the 20-second monotonic deadline. On expiry:
1. request `kill()`;
2. discard every output byte without waiting synchronously for exit;
3. quarantine the child and pipe threads, and prevent another launch until
   the child is reaped and all pipe threads finish;
4. return `ProbeOutcome::Unavailable(MetadataError::WorkerFailed)`.

The worker's own hard timer also terminates it if the parent process exits
before the parent watchdog can act.

After normal exit, require success exit status, exactly one bounded reply, current generation, and later parent inventory validation.

- [ ] **Step 4: Replace parent direct queries**

In `inspect_discovered_devices()`, keep the current order:
1. reject elevated parent;
2. register interface-class PnP notifications;
3. enumerate records;
4. capture `ScanSnapshot`;
5. call `probe_via_worker` per record;
6. re-enumerate;
7. validate snapshot/device numbers;
8. unregister notifications;
9. validate generation/snapshot again.

Delete the old direct-device query path only after worker tests are green.

- [ ] **Step 5: Verify GREEN/full suite**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace --all-targets`
- `./scripts/check-windows-sdk-abi.ps1`
- `npm run build`
- `npm run test:main`

Expected: all pass locally/on Windows CI.

- [ ] **Step 6: Commit**

`git commit -am "feat(p2-i2c): isolate metadata probes behind worker watchdog"`

---

### Task 7: Security regression checks and CI acceptance

**Files:**
- Modify only if tests expose gaps: `crates/windows-agent/src/*.rs`, `tests/windows/drive_layout_abi.c`
- No capability enablement files may change to true.

**Interfaces:**
- No new public interface; this task proves the previous tasks satisfy the spec.

- [ ] **Step 1: Add explicit negative/source-boundary tests**

Tests/assertions must prove:
- worker request has no IOCTL field;
- normal CLI help does not advertise `__metadata-worker`;
- renderer/main-process IPC tests still expose no physical-disk method;
- source contains no `ReadFile`, `WriteFile`, `IOCTL_DISK_SET_DRIVE_LAYOUT_EX`, format, mount, repair, or elevation request in the new path;
- worker reply after cancellation/PnP invalidation cannot become `Ready`.

- [ ] **Step 2: Run the complete required PR verification**

Required five jobs:
1. Rust safety tests Windows;
2. Rust safety tests Ubuntu;
3. Electron TypeScript/renderer build + IPC tests;
4. Windows Electron UI preview;
5. DiskSpd XML integration.

Do not merge with any required job red or pending.

- [ ] **Step 3: Code review the branch**

Review specifically:
- all `OVERLAPPED`/event/buffer lifetimes;
- `GetLastError` sampled immediately after failing Win32 calls;
- exact-request `CancelIoEx`;
- no HANDLE close under pending kernel I/O;
- no callback wait/block;
- no unbounded pipe allocation;
- no path in argv/log/stdout;
- all worker abnormal exits fail closed.

- [ ] **Step 4: Merge the code PR and verify fresh main five-for-five**

Squash merge only after required PR checks pass. Then verify the same five jobs on the resulting `main` SHA before claiming item 2 complete.

---

### Task 8: Final checkpoint documentation

**Files:**
- Modify: `docs/P2_I2C_HANDLE_REMOVAL.md`
- Create: `docs/P2_I2C_BOUNDED_IO.md`
- Modify: `AGENTS.md`
- Modify: `docs/SESSION_HANDOFF.md`
- Modify: `docs/PLAN.md`
- Update: GitHub Issue #31

**Interfaces:**
- Documentation only.

- [x] **Step 1: Record exact merged SHA and CI evidence**

State precisely what is implemented:
- 5s overlapped IOCTL budget;
- exact-request cancellation;
- 1s grace;
- disposable worker;
- 20s parent hard deadline;
- parent post-worker generation/inventory validation.

- [x] **Step 2: Preserve explicit limitations**

Still open after item 2:
- strong hardware-backed identity;
- physical Windows 11 USB/NVMe/SATA/BitLocker/512e/4Kn acceptance;
- authenticated read-only Electron-agent IPC;
- real disk list UI;
- all write/format/mount/repair/resize work.

Do not claim physical hotplug acceptance from hosted CI.

- [ ] **Step 3: Docs PR five-for-five and merge**

Run/await the same five required checks, merge the documentation PR, then verify fresh `main` five-for-five once more.

- [ ] **Step 4: Update Issue #31**

Mark roadmap item 2 complete only after the final post-doc `main` verification. Keep Issue #31 open for hardware identity and later acceptance/security gates.
