# P1 — Windows 11 adaptive UI acceptance matrix

Status: implementation started. This document is a test protocol, NOT a
claim that real monitor testing has already passed.

## Foundation

- Chromium/Electron supplies platform-aware display scaling.
- Electron main samples the current display via `screen.getDisplayMatching`
  and publishes its **numeric** `scaleFactor` using a narrow IPC API.
- The renderer records `devicePixelRatio` for comparison; it must not
  manually multiply all CSS pixels by the monitor scale factor (double scaling).
- Native titlebar/window borders remain enabled for correct Windows sizing.
- Windows GUI remains non-elevated; Rust agent is disconnected.
- System and explicit light/dark themes use Fluent UI design tokens.
- Resizing/moving the window causes a display-status refresh only.

## Manual Windows 11 acceptance tests

| Case | Test setup | Required result |
|---|---|---|
| DPI-01 | 100% on 1920×1080 display | Controls and text visible, no clipping |
| DPI-02 | 125% laptop display | Crisp rendering and consistent hit targets |
| DPI-03 | 150% display | Responsive cards and no horizontal overflow |
| DPI-04 | 175% display | Headers, buttons and sidebar reachable |
| DPI-05 | 200% (4K typical) | Crisp text, focus rings, no overlapping content |
| DPI-06 | 250% display | Keyboard-only navigation and scrolling |
| DPI-07 | 300% display | Readable content, safe minimum window bounds |
| DPI-08 | Mixed 100% / 200% dual-monitor | Move window between monitors, status scale updates |
| DPI-09 | Windows scaling changed while open | No stale overlays or incorrect text sizes |
| A11Y-01 | Windows Narrator | Announces headings, status, theme and disabled actions |
| A11Y-02 | Keyboard only | All enabled controls reachable, visible focus |
| A11Y-03 | Windows contrast themes | All text and focus state legible |
| A11Y-04 | Reduced motion | No essential animation required |
| THEME-01 | System light/dark auto | Fluent theme follows OS |
| THEME-02 | Manual light and dark | Preference persists without disk permissions |

## Automated gates (P1)

- IPC sender validation and sandbox-preload contract tests.
- No exposed generic `ipcRenderer`, shell or file-system APIs.
- Production CSP blocks remote and loopback connections.
- TypeScript check, renderer bundle and Windows CI pass.
- End-to-end packaged executable smoke test is still **pending**.

## Limitations

Actual per-monitor DPI mode is ultimately controlled by Electron/Chromium
and Windows. A percentage readout is not proof of Per-Monitor-V2 manifest
semantics; packaged Windows manifests and mixed-display transitions must be
inspected before marking P1 complete.
