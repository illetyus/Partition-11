# SDD ledger — plan: docs/superpowers/plans/2026-10-21-windows-test-candidate-completion.md

## Setup — 2026-10-21
- User instruction: apply the full test-candidate completion plan; user owns physical Windows 11 acceptance.
- Authorization inherited from the approved P2.5 plan: feature-branch commit/push and draft PR #53 for CI are allowed; merge is not.
- Worktree: `D:\Partition 11\.worktrees\readonly-disk-preview`; branch `feature/p2.5-volume-eligibility-policy`; baseline `38a0d7597602bd95b8980a04458ceea10caed67b`.
- Preserve all prior work, including journal/recovery changes and generated proposal docs. No local DiskSpd run or physical inventory/test.
- Tool constraint/ruling: prior SDD ruling authorizes PowerShell/apply_patch equivalents because packaged Bash helpers are denied. Use task-local Rust 1.99.1/MSVC 14.50.
- Live baseline: draft PR #53 open on this branch. At baseline SHA, CI and UI workflows pass; DiskSpd workflow fails. Prior PR description is stale and will be updated after corrected fresh CI evidence.
- Task 0: complete. The pinned Rust helper compile probe failed before the fix with exit 1 and the expected missing APIs: `recover_test_journal` (5 references), `create_journaled_for_test`, `execute_lease` (4 references), and `BenchmarkExecutionError` (2 references). Captured from the task-local Rust 1.99.1/MSVC 14.50 environment; prior dirty state and PR authority are preserved above.

## Preflight interface map
- Task 1 -> Task 2: `CompletedBenchmark`, durable `running`, exact lease cleanup, conservative uncertain-error behavior. Task 2 may not start until Gate A passes.
- Task 2 -> Task 3: bounded newline request plus open cancel/EOF channel; exact final response with request ID; sanitized result statuses; fixed 120 s agent watchdog.
- Task 3 -> Task 4: preview/ready availability, existing start/cancel request contract, sanitized progress/results and stale-selection status; main owns consent and process quarantine.
- Task 4 -> Task 5: real eligible volume selection uses only current opaque token; demo fixture cannot become a measured report.
- Task 5 -> Task 6: runtime-disabled whole pipeline and all safety gates pass on a named SHA.
- Task 6 -> Task 7: activation commit identity and successful checks bind to the package artifact.
- Ruling: run agent protocol tests against synthetic scan/file seams, never invoke live inventory for integration tests. A CI temporary file proves engine/handle compatibility only, never user-device performance.
- Ruling: proposal addendum is now approved by user instruction “Uygula”; implement its one-GiB initialization disclosure and revised deadlines. If the pinned engine cannot justify an enforceable byte bound, keep runtime disabled and revise the math before activation.

## Task status
- Task 0: complete — exact RED recorded above.
- Task 1: complete; Gate A passed at `41b05cb6887d29df67396a55bcba12cc3bf97be3`. Windows worker suite at that checkpoint: 49 passed, 3 intentionally ignored. Fresh pinned DiskSpd XML integration run `39539601092`, CI run `39539601099`, and Windows UI run `39539601088` all completed successfully. The DiskSpd workflow exercised the exact pinned binary on disposable CI storage inside the contained Job Object, confirmed exact cleanup after drain, and validated XML. No local DiskSpd or physical-device test was run.
- Task 2: implementation and local package verification complete; commit and fresh CI pending. Added one-run agent dispatch, bounded request/final frames, live stdin cancel/EOF handling, non-elevated/fresh-target revalidation, a PnP generation guard retained through cleanup, a fixed packaged-engine path, 1-GiB random initialization with bounded chunking/throttle/flush/deadline, and the contained worker lifecycle. Regression coverage includes real open-pipe request/cancel and EOF, invalid authorization/stale/ineligible targets with zero start side effects, a PnP change injected after revalidation that blocks the start side effect (mutation-checked), preparation and late-I/O behavior, and a temporary-file ready-to-running/process-drain/exact-cleanup integration. Locked Windows tests: agent 131 passed / 2 live-device tests ignored; worker 54 passed / 3 CI/process fixtures ignored. `cargo fmt --all -- --check`, strict targeted Clippy, and Windows x64 release build passed. No production `benchmark-run`, physical inventory, user-volume write, or local DiskSpd invocation was run. The runtime gate remains false. Main-process transport/controller/recovery changes are still separate dirty Task 3 work.
- Task 3: pending.
- Task 4: pending.
- Task 5: pending.
- Task 6: pending.
- Task 7: pending.
