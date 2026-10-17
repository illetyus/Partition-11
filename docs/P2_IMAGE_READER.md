# P2-I1 — bounded image reads

Status: Internal Rust adapter implemented in source; **not wired to the Electron
app or Windows agent**. It does not enable disk inspection in the GUI.

`crates/core/src/image.rs` accepts a separately trusted **absolute, ordinary
regular file path** and opens it using read-only flags. Final-component
symlinks, directories, empty files, and Windows raw device / UNC namespace
paths are rejected. Requests must be nonempty and at most 4 MiB, use checked
byte offsets, and reject reads beyond the captured file length. Only 512- and
4096-byte sector reads are supported. Length changes during reads fail closed.

Tests exercise read ranges, 512/4Kn sectors, invalid geometry, overflow,
symlinks (Unix), size changes and ordinary-file rejection; GitHub Actions
runs them on Linux and Windows.

## Not a production disk lease

- This adapter is an internal **file reader**, not a trusted path-selection API.
  Canonical parent directories, Windows reparse points, file identity
  handles, hard links, concurrent same-length mutations and exclusive
  immutable snapshots require a later P2 agent authorization layer.
- A regular image can contain arbitrary corrupt bytes; parsers must still
  validate all bounds and checksums independently.
- The renderer and IPC do not expose this reader, so the reported
  `inspectImages` capability remains **false**.
- No physical disk discovery, raw storage handles, write operations, UAC
  elevation, formatting, repair or benchmark runtime are added.
- Hardware-dependent verification and genuine fixture/oracle comparisons
  remain mandatory before P2 acceptance.

Next steps: tie image sectors to the existing pure GPT/MBR parser through a
versioned internal interface, add independent genuine fixtures, and implement
Win32 read-only disk discovery as a **separate**, authenticated adapter.
