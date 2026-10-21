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
- Task 1: implementation complete; Gate A CI pending — execution lifecycle, test-only journal policy, replacement/wrong-size/hard-link recovery tests and pinned CI workflow are implemented locally. Windows worker suite: 49 passed, 3 intentionally ignored. Strict targeted Clippy (`-D warnings`) and `cargo fmt --all -- --check` pass. First fresh GitHub run (39538874608) exposed hosted-runner file ownership defaulting to Administrators; the implementation now writes the current-user owner SID explicitly in the protected descriptor. The next fresh Windows job passed that ACL fix. Its DiskSpd Rust Job Object/lease integration also passed, then the Node XML check exposed a missing `measurementSeconds` field in the planner result. Added a failing assertion, added the field to the planner contract, and verified `npm run build:main` plus targeted benchmark policy tests (21/21) outside the AppContainer. Fresh GitHub rerun with both fixes is pending.
- Task 2: pending.
- Task 3: pending.
- Task 4: pending.
- Task 5: pending.
- Task 6: pending.
- Task 7: pending.
