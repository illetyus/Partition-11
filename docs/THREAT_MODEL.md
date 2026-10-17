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

## P2.5 benchmark threat model

| Risk | Preventive control |
|---|---|
| DiskSpd raw device writes or overwrite of an existing file | Only an app-created exclusive scratch file may be targeted. Never pass raw disk, drive-letter-only or arbitrary user target paths |
| CLI/XML command injection | Predefined typed profiles and direct process launch without a shell |
| Symlink/junction/reparse-point or device swap | Validate opened handle, reparse policy and stable volume identity before launching |
| Wear, thermal load, disk exhaustion | Bound test file size, repetitions, duration and estimated bytes written, require explicit consent |
| Child process outliving cancellation | Kill the owned process tree/Job Object, enforce strict timeout |
| Crash leftovers or accidental cleanup of user data | Track exact owned file identity, no wildcard or recursive deletions |
| Supply-chain tampering | Signed/reproducible provenance where possible, pinned version and SHA-256, no runtime executable downloads |
| Misleading metrics | Report file-backed scope, cache mode, workload, filesystem, engine version, sample count |

Benchmark writes are ordinary filesystem writes to a disposable, application-
owned file; they do not grant raw-write privileges. The P5 raw-disk policy
remains unchanged.

## Required review before P5

Implement and test Windows raw storage access, safe privilege transitions,
device locking, documented cancellation semantics, rollback limitations,
and irreversibility warnings. Owner approval alone is not a substitute
for those technical checks.
