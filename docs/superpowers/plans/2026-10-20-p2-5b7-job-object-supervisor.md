# P2.5B.7 — Bounded Windows process-tree supervisor Implementation Plan

> For agentic workers: use superpowers:executing-plans to implement this plan task by task. Steps use checkbox syntax.

Goal: Add an internal Windows process supervisor for a future file-backed DiskSpd worker, with bounded output, deadline/cancel handling, and whole-job termination, while keeping benchmark launch disabled.

Architecture: The benchmark-worker crate creates an anonymous Windows Job Object, sets kill-on-last-handle-close and a small active-process cap, and uses `PROC_THREAD_ATTRIBUTE_JOB_LIST` so the child is already in the job when `CreateProcessW` returns it suspended. `STARTUPINFOEX` also carries an explicit stdio handle allowlist. The supervisor enforces a fixed wall deadline, cancellation, and stdout/stderr byte caps. It does not select a volume, authorize writes, launch DiskSpd, expose IPC, or enter the Electron package.

Tech Stack: Rust 2021, raw Win32 FFI confined to a Windows-only module, existing Windows SDK toolchain, Node-independent Cargo tests.

Spec: docs/P2_5_DISK_BENCHMARK.md (P2.5B file-only scope, fixed bounded runs, output bounds, cancellation of the complete process tree, no production launch before remaining gates).

## Global Constraints

- Only a fixed allowlisted executable and fixed argument combinations may reach this supervisor; there is no shell.
- Every operation uses one application-owned file target; raw disk and raw volume targets are refused before launch.
- Each child process has a measured-work timeout of at most 75 seconds, a fixed 15-second termination grace, stdout capped at 1 MiB, stderr capped at 16 KiB, and an active-process limit of 4. Repetitions are separate sequential child runs.
- Cancellation and timeout terminate the complete owned process tree before its scratch lease is cleaned.
- Benchmark launch remains disabled until consent, target ownership, identity, result validation, CI and hardware gates pass.
- Never run DiskSpd or query/write a physical volume in this implementation task.

## Review Focus

- An executable path containing spaces must resolve as exactly one application path, not a prefix executable.
- Quotes and backslashes in arguments must round-trip through Windows command-line parsing without changing argument boundaries.
- Job-list attribute or process-creation failure in a nested or restricted parent-job environment must fail closed; there is no fallback that runs the worker outside the owned job.
- Timeout or cancellation racing normal child exit must never return success while a descendant remains in the job.
- A stdout/stderr flood or a pipe that remains open must hit a byte/deadline bound and terminate the whole job.
- If termination does not drain within its grace period, the caller must retain the lease and report failure rather than claim cleanup is safe.

---

### Task 1: Contained suspended process creation

Files:
- Create: crates/benchmark-worker/src/job_process.rs
- Modify: crates/benchmark-worker/src/lib.rs
- Test: Windows unit tests in job_process.rs

Interfaces:
- Consumes: the existing Windows-only benchmark-worker crate; no public or Electron-facing API.
- Produces: crate-private ProcessCommand with private executable/argument fields and a test-only fixture constructor, encode_command_line(executable: &OsStr, args: &[OsString]) -> Result<Vec<u16>, ProcessError>, ProcessLimits::from_workload(warmup_seconds: u32, measurement_seconds: u32) -> Result<ProcessLimits, ProcessError>, ProcessOutput { exit_code: u32, stdout: Vec<u8>, stderr: Vec<u8> }, and run_process(command: ProcessCommand, limits: ProcessLimits, cancel: &AtomicBool) -> Result<ProcessOutput, ProcessError>. Tests use a local observer seam for native launch milestones. No production command constructor is added in this plan. ProcessError distinguishes invalid command, timeout, cancellation, output limit, undrained job, and underlying I/O failure.

- [x] Step 1: Add failing Windows tests for absolute executable validation, command-line quoting with spaces/quotes/backslashes, workload-limit rejection, zero/nonzero exit propagation, and a test-only launch observer that confirms creation suspended in the job, then Resumed.
- [x] Step 2: Run cargo test --locked -p partition11-benchmark-worker on the Windows host and confirm the new supervisor API/tests fail because the implementation is absent.
- [x] Step 3: Implement private Win32 handle wrappers, a bounded Windows argument encoder, ProcessLimits::from_workload enforcing 0..=15 seconds warmup and 2..=60 seconds measured time, a 15-second termination grace, 1 MiB stdout, 16 KiB stderr and an anonymous Job Object with KILL_ON_JOB_CLOSE plus active-process cap 4. Use `STARTUPINFOEX`, `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` for the explicit NUL/stdout/stderr allowlist, and `PROC_THREAD_ATTRIBUTE_JOB_LIST` to place the suspended child in its job during process creation. Resume only after successful creation; any failure after creation terminates the contained child/job and fails closed.
- [x] Step 4: Run the targeted worker tests and confirm process launch and quoting tests pass. Do not invoke DiskSpd.
- [x] Step 5: Run cargo fmt --all -- --check and cargo clippy --locked -p partition11-benchmark-worker --all-targets -- -D warnings.

### Task 2: Bounded output, deadline, cancellation and process-tree drain

Files:
- Modify: crates/benchmark-worker/src/job_process.rs
- Test: Windows unit tests in job_process.rs
- Modify: docs/P2_5_DISK_BENCHMARK.md and docs/SESSION_HANDOFF.md

Interfaces:
- Consumes: Task 1 ProcessCommand, ProcessLimits, run_process, and Job Object ownership.
- Produces: ProcessOutput with exit code and bounded stdout/stderr, and typed timeout, cancellation, output-limit, undrained-job, and I/O failures.

- [x] Step 1: Add failing tests for timeout, cancellation, stdout and stderr overflow, and a helper child process that remains in the job after its parent exits.
- [x] Step 2: Run those tests first and verify they fail on the current implementation for the named missing behavior.
  Test evidence: timeout/cancel checks already passed because Task 1 included deadline and job abort semantics; stdout/stderr capture and overflow checks failed as expected before Task 2.
- [x] Step 3: Implement bounded pipe draining with PeekNamedPipe/ReadFile, a measured-work deadline derived from the validated workload, cancellation polling, TerminateJobObject on every abort path, and wait for the Job Object to become empty within the fixed 15-second grace. If the job is not empty, return ProcessError::Undrained and preserve ownership handles so drop closes the kill-on-close job.
- [x] Step 4: Run targeted tests; verify a descendant is no longer active before an aborted run is reported as drained.
- [x] Step 5: Document the boundaries and current limitations. Keep benchmark IPC, worker packaging, user consent, volume access and DiskSpd invocation disabled.
- [x] Step 6: Run cargo fmt --all -- --check, cargo clippy --locked -p partition11-benchmark-worker --all-targets -- -D warnings, cargo test --locked -p partition11-benchmark-worker, and cargo build --locked --workspace --release.

### Completion boundary

This plan completes only the internal process-tree primitive. It does not complete P2.5 acceptance, create a production benchmark worker, enable UI launch, authorize writes, validate physical disks, package DiskSpd, or replace the user's Windows 11 hardware test.

Commit ruling: The worktree already contains approved but uncommitted P2.5B.4–B.6 changes, including overlapping benchmark-worker and package metadata. Do not make a partial commit that accidentally includes unrelated staged hunks; keep this implementation in the same reviewable local feature set until the whole branch is reviewed.
