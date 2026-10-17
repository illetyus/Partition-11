# Windows 11 interface — final UI acceptance checklist

This document distinguishes **implemented** user interface features from
**verified** Windows behavior. It is not an assertion that disk operations
exist.

## Reference layout

- Product title: Partition 11; screen title: Biçimlendir.
- Native Windows caption overlay and custom draggable top bar.
- Sidebar shows exactly identified synthetic disk cards with selected
  highlight; sample Windows/system disk is protected and disabled.
- Selected drive summary with bus, layout, size, filesystem and usage.
- EXT4 label uses a 16-byte UTF-8 validation and visible help text.
- Advanced settings accordion can edit draft block size and reserved space.
- Destructive-operation warning and administrator privilege notice.
- Primary **Biçimlendir** action is disabled until a future real backend
  provides a safe capability, independently authorized by a disk agent.
- Footer explicitly states that the preview has no access to real disks.

## Theme, responsiveness, accessibility

- Theme switches are applied to Fluent and CSS using a data-theme attribute.
- Native Windows caption colors follow the selected light/dark theme via
  an allowlisted IPC handler (which validates sender and theme payload).
- Keyboard focus rings, heading structure, explicitly disabled actions,
  labelled disk selectors and advanced controls are present.
- At viewport widths under 800px, disk navigation scrolls horizontally.
- At viewport widths under 560px, forms stack into one column.
- Actual Per-Monitor-V2 manifest behavior, 100–300% DPI, Narrator and
  Windows high contrast **still require human verification**.

## GitHub-hosted Windows verification

Windows UI Actions use real Electron to open the **built renderer** with the
**compiled sandbox preload**, exercise disk selection, advanced options,
light/dark theme and responsive layout, then capture PNGs as artifacts.

This test harness uses only mock IPC diagnostics and fixed sample disks; it
does not request administrator privileges or access actual devices.

Screenshots:
- `01-light-reference.png`
- `02-advanced-sandisk.png`
- `03-dark-reference.png`
- `04-narrow-desktop.png`
- `05-narrow-layout.png`

The smoke harness is stored under `tests/` and excluded from packaged
application files. It should fail CI if Electron cannot start, if a screenshot
cannot be captured, or if display validation fails. The screenshot test
cannot replace testing the actual packaged .exe on a multi-monitor Windows 11
machine; CI runners typically present only a virtual screen.

## Safety release gate

Do not mark storage operations complete based on screenshot tests. Format,
repair, resize, disk-identity selection, UAC and drive locks have separate
future P2–P8 release gates.
