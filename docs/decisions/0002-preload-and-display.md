# ADR-0002 — Sandbox preload and per-display scale observation

Status: Accepted for P1 preparation, 2026-10-17

## Problem

Sandboxed Electron preloads cannot freely `require()` local CommonJS
modules. A TypeScript build succeeding does not prove that a packaged
preload works. Electron/Chromium manages per-monitor scaling; blindly
multiplying CSS dimensions by the monitor scale factor causes double-scaling.

## Decision

1. Preload imports the `Partition11Bridge` TypeScript type only. Channel
   names are local literals, checked by a generated-code contract test.
2. Main-process IPC is allowlisted by sender frame, webContents, exact
   renderer URL and a versioned API.
3. Expose only display scale factor, work area and high contrast status.
   No disk paths, device handles, raw IPC APIs or local filesystem APIs.
4. Main observes window move/resize and system display-metric changes.
5. Electron maintains the native caption and Windows manages actual DPI.
6. The build output's production CSP must not allow HTTP/WS loopback;
   development HMR allowances are injected only by Vite dev server.

## Caveat

This does not alone establish a passing Per-Monitor-V2 mode. Actual Windows
DPI transition, compiled manifest, high-contrast and accessibility checks
remain manual acceptance gates documented in docs/P1_WINDOWS11_UI.md.
