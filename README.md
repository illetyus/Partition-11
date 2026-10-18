# Partition 11

Modern, Windows 11–oriented ext2/ext3/ext4 disk management desktop application.

**Status (2026-10-18):** P0/P1 preview UI and P2 bounded GPT/MBR/image primitives exist. P2-I2a added native SetupAPI enumeration; a **P2-I2b development-only, non-elevated metadata diagnostic** is being validated separately (see [metadata safety and hardware-test matrix](docs/P2_WINDOWS_DISK_METADATA.md)). It queries device number, capacity, sector geometry and bus type without reading raw sectors or exposing device paths. **No user-facing disk scan, live benchmark, raw disk reads/writes, formatting, repair or resize is enabled.** P2-I2b [#22](https://github.com/illetyus/Partition-11/issues/22) remains open for layout and hardware acceptance.

Architecture: Electron + React + TypeScript + Fluent UI (desktop shell), isolated Rust core (operation policies and future filesystem adapters), Windows privileged agent (future; explicitly disabled). Linux/WSL is not a runtime prerequisite.

**Continue work in a new ChatGPT/Codex session:** read [the current handoff and exact next step](docs/SESSION_HANDOFF.md) and [agent working rules](AGENTS.md) first. The handoff includes completed PRs, green CI evidence, remaining tasks, the next P2-I2 issue, and the mandatory tested-PR-to-`main` merge sequence.

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

## UI preview on GitHub Windows Actions

The Windows UI workflow runs the compiled Electron renderer with the compiled
sandboxed preload and synthetic, **clearly marked sample devices**. It tests
sample-disk navigation, native theme selection, the advanced-settings panel,
responsive sizes, and the permanently disabled format action. Successful runs
publish light/dark/compact PNG screenshots plus a Windows unpacked preview
archive. They do not authorize disk reads/writes or establish true multi-monitor
DPI compatibility.

See [UI acceptance matrix](docs/P1_UI_ACCEPTANCE.md) and
[Windows UI Actions](.github/workflows/windows-ui.yml).

The UI preview is not a functional disk formatter; physical-device operations
require later separate safety-gated development.
