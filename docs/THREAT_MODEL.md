# Threat model — Phase 0 / planned privileged operations

## Assets

Physical disks, partition tables, ext journals/inodes, personal files,
disk serials, Windows admin privileges, release/signing credentials.

## Trust boundaries

- Renderer is untrusted presentation; never grant Node.js or disk handles.
- Main IPC receiver must authenticate frame origin and validate message types.
- Agent in later phases must mistrust Electron main and reject unexpected
  commands. Privileged agent is absent in P0.
- Third-party filesystem parsers handle attacker-controlled bytes. Corrupt
  filesystem images must be assumed hostile.

## Primary threats and mitigations

| Threat | Mitigation |
|---|---|
| Renderer compromise to privileged write | sandbox/contextIsolation; narrow IPC; independent agent checks |
| Wrong disk due to hotplug/drive-letter recycling | opaque snapshot + re-enumeration + fail on mismatch |
| Concurrent filesystem writes | exclusive volume lock; abort if cannot obtain |
| Bounds overflow / sector mismatch | checked offsets, 512e/4Kn tests, no unchecked casts |
| e2fs/FFI corrupt-image exploitation | isolated process, fuzzing, no raw parsing in renderer |
| Malicious updates / npm dependency compromise | lockfiles, pinned actions, SBOM, signatures, dependency reviews |
| Command injection | structured protocol; no shell invocation |
| Data exfiltration | no telemetry, no external content, blocked web navigation |
| False assurance from image tests | physical disk gate requires dedicated destructive test devices |

## Required review before P5

Implement and test Windows raw storage access, safe privilege transitions,
device locking, documented cancellation semantics, rollback limitations,
and irreversibility warnings. Owner approval alone is not a substitute
for those technical checks.
