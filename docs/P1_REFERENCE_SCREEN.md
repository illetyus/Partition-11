# P1 — Disk formatting screen (reference UI implementation)

Status: Electron/React source implementation for UI preview; hardware-backed disk
inventory, formatting and storage operations remain disabled.

This screen follows the user-provided Windows 11 UI reference from 2026-10-17:
two-column layout, header, disk selector at left, format settings at right,
partition usage bar, advanced settings, high-visibility destructive-operation
warning, privilege notice, and bottom status bar.

## Explicit behavior in the preview

- App and screen are distinct: product **Partition 11**, screen **Biçimlendir**.
- Mock disks are explicitly labeled **Örnek veriler**. No fake data is
  presented as attached hardware.
- Mock Windows/system disk is visibly protected and not selectable.
- Selecting a sample device updates its overview and partition usage.
- Editable volume label validates **UTF-8 encoded bytes** (16-byte limit).
- Advanced settings can expand/collapse and change draft field values.
- Light/dark theme toggles in the header; system theme is the initial fallback
  and preferences persist locally when browser storage is available.
- **Biçimlendir** stays disabled even with a valid label: there is no disk
  agent integration, privilege escalation or formatting capability.
- Bottom status bar states that real disks are not accessible.

## P1 manual acceptance gates

The PR's Windows GitHub Actions verifies TypeScript compilation and source
policy tests; this does not prove a Windows desktop executable runs at 100%,
125%, 150%, 175%, 200%, 250%, or 300% mixed-monitor scaling.

Verify on a Windows 11 packaged build:
- Native caption controls, drag region, focus behavior, mixed-DPI move.
- Fluent/Segoe UI variable font appearance in light and dark modes.
- Keyboard-only access, high-contrast and Narrator, reduced motion.
- Sidebar vertical/horizontal scrolling when many disks are eventually shown.
- UTF-8 labels with accented chars, duplicate disk IDs, long capacity strings.
- No enabled destructive operation on sample or unverified devices.

This PR changes the **presentation layer only**. GPT/MBR parser remains
a memory-only Rust decoder and is not connected to the user-facing list.
