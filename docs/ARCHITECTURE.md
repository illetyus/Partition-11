# Architecture and trust boundaries

```text
 Electron renderer (React/Fluent UI, untrusted presentation)
          |
   typed preload facade (allowlisted methods)
          |
 Electron main (event origin + schema validation; NO raw disk operations)
          |
   planned authenticated, versioned local IPC (not yet connected)
          |
 partition11-agent (Rust, future separate elevated process, fail-closed)
          |                           |
   image I/O adapter          Win32 storage adapter (future)
          |                           |
  filesystem engine             physical devices
```

The Rust agent remains a separate diagnostic executable and is not invoked
by Electron. P2-I2a counts SetupAPI disk interfaces without opening devices;
a P2-I2b diagnostic-only path can open agent-discovered handles with GENERIC_READ
for bounded metadata IOCTLs when the process is non-elevated. It never reads
raw sectors, grants no write access, and does not authorize UI inspection.
No stable device identity is established. All displayed capability flags
must reflect actual code, not design intention.

## Why not expose libext2fs directly to renderer?

A compromised renderer would inherit access to memory-unsafe filesystem
code and possibly a privileged disk handle. Instead, use narrow schema-
checked messages and keep native storage in a dedicated process.

## Device identity protocol (P2 design, not implemented)

Never accept a device name/path alone for a destructive operation.
P2 will issue an ephemeral opaque ID and immutable discovery snapshot:
bus, model, serial if available, sector size, capacity, layout hash, target
start/length, media generation, and relevant unique device identifiers.
The agent re-enumerates and compares before opening a write handle. A
mismatch invalidates the plan (prevents drive-letter reuse/TOCTOU).

## Operations (future)

1. Scan (read-only)
2. Preview and validate a prospective operation
3. Confirm a human-readable plan tied to the device snapshot
4. Require elevated agent when needed; recheck all invariants
5. Acquire Windows volume locks/dismount only if safe
6. Execute bounded transaction with progress and explicit fault handling
7. Flush, re-enumerate, independently verify; never imply atomic rollback

## Licensing boundaries

Electron, Fluent UI React and project source: MIT. Rust engine candidate:
MIT. The combined GPLv2 e2fsprogs tools are NOT linked or vendored in P0.
IPC separation is an architectural boundary, not guaranteed legal license
separation. See docs/LICENSES.md.
