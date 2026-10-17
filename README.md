# Partition 11

Modern, Windows 11–oriented ext2/ext3/ext4 disk management desktop application.

**Status:** Phase 0 — foundation in progress. **No physical disk writes are implemented or enabled.**

Architecture: Electron + React + TypeScript + Fluent UI (desktop shell), isolated Rust core (operation policies and future filesystem adapters), Windows privileged agent (future; explicitly disabled). Linux/WSL is not a runtime prerequisite.

Read [the project plan](docs/PLAN.md), [architecture](docs/ARCHITECTURE.md), [security policy](SECURITY.md), and [third-party license inventory](docs/LICENSES.md) before implementing filesystem operations.

> Safety: This is experimental storage-management software. Do not use unverified builds on disks containing valuable data.
