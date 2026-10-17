# Security policy

Partition 11 is pre-release, experimental disk-management software. No
data-integrity assurances are provided. Do not point unfinished builds at
real disks containing valuable data.

## Supported scope

- Phase 0: GUI scaffold, Rust policy library, status-only diagnostic stub.
- No raw disks are opened. No disk writes, mounts, repairs, or formats exist.
- Report vulnerabilities privately through GitHub's private vulnerability
  reporting feature if enabled; otherwise contact the maintainers without
  publishing device details or proof-of-concept destructive payloads.

## Invariants

1. Renderer: `sandbox: true`, `contextIsolation: true`,
   `nodeIntegration: false`, restrictive CSP and no external navigation.
2. Preload exposes specific allowlisted methods, never raw `ipcRenderer`.
3. Main validates origin, frame, schema and authorization for all IPC.
4. No UAC elevation for ordinary GUI tasks. Future disk agent must be
   separate, signed, least-privileged, and audited.
5. Future physical disk writes require: device re-identification; allowed
   partition range; successful exclusive access; non-system/non-boot target;
   an explicit plan + confirmation; verified size/sector alignment; safe
   abort semantics.
6. Unknown ext4 incompat/ro_compat features must prevent unsupported writes.
7. Private disk contents, volume names, serials and paths do not enter
   telemetry. No telemetry by default.
8. Never run shell commands assembled from user-controlled strings.

## Phase 0 threat model

See [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md). Later phases must update
the threat model before acquiring any storage write capabilities.
