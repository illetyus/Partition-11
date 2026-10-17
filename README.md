# Partition 11

Modern, Windows 11–oriented ext2/ext3/ext4 disk management desktop application.

**Status:** Phase 0 foundations implemented; Phase 1 Windows 11 UI work in progress. **No disk reads, physical disk writes, or repair/format operations are implemented or enabled.**

Architecture: Electron + React + TypeScript + Fluent UI (desktop shell), isolated Rust core (operation policies and future filesystem adapters), Windows privileged agent (future; explicitly disabled). Linux/WSL is not a runtime prerequisite.

Read [the project plan](docs/PLAN.md), [architecture](docs/ARCHITECTURE.md), [security policy](SECURITY.md), and [third-party license inventory](docs/LICENSES.md) before implementing filesystem operations.

> Safety: This is experimental storage-management software. Do not use unverified builds on disks containing valuable data.

## Development

For a local development environment, install Node.js 24 and a current stable
Rust toolchain. Run `npm ci`, `npm run build`, `npm run test:main`, and
`cargo test --locked --workspace`. To launch the Electron GUI locally,
install the full Electron runtime and run `npm run dev` on Windows.
CI's `ELECTRON_SKIP_BINARY_DOWNLOAD` option is only for source compilation
and is **not** an end-user installer smoke test.

See [P1 Windows 11 acceptance tests](docs/P1_WINDOWS11_UI.md) and
[reproducible build policy](docs/P0_REPRODUCIBLE_BUILDS.md).
