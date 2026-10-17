# ADR-0001: Electron UI with isolated Rust operation backend

Status: Accepted for Phase 0, 2026-10-17

## Context

The product must look native to Windows 11, support responsive layout,
mixed-monitor DPI scaling, and ultimately operate on raw storage safely.
The user explicitly chose Electron. GPL license compatibility and
untrusted filesystem input are core architectural risks.

## Decision

Use Electron + React + TypeScript + Fluent UI React for UI. Use a narrow,
sandboxed preload API. Develop an independent Rust core for capability
and operation policy. Create a separate Windows privileged helper only
when required and only after read-only device discovery tests pass.
Evaluate an MIT ext4 engine; do not ship it without independent oracle
and fault-injection tests.

## Consequences

- Higher bundled runtime size than native UI; easier UI development.
- Two runtime/language toolchains; clear trust boundary.
- P0 disables all disk I/O.
- GPLv2 e2fsprogs code is not automatically copied or linked.
- Native APIs needed for Per-Monitor V2 awareness must be verified
  against actual packaged Electron executable on Windows 11.
