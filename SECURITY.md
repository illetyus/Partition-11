# Security policy

Partition 11 is pre-release, experimental disk-management software. No
data-integrity assurances are provided. Do not point unfinished builds at
real disks containing valuable data.

## Supported scope

- Phase 0: GUI scaffold, Rust policy library, status-only diagnostic stub.
- P2: only a memory-only partition-table parser. P2.5: benchmark policy/XML
  decoder and CI-only owned scratch-file tests, not an end-user benchmark.
- The separate Windows Rust agent may open a newly discovered disk interface with **GENERIC_READ for metadata-only IOCTLs**, but never reads raw sectors or writes disk data. It rejects elevated token execution. No UI disk access, mounts, repairs, or formats exist. See docs/P2_WINDOWS_DISK_METADATA.md.
- Report vulnerabilities privately through GitHub's private vulnerability
  reporting feature if enabled; otherwise contact the maintainers without
  publishing device details or proof-of-concept destructive payloads.

## Invariants

1. Renderer: default-deny web permission requests and permission checks;
   deny navigation, new windows and webview attachment. Keep renderer
   `sandbox: true`, `contextIsolation: true`,
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
