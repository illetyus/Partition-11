"use strict";

/**
 * Real Chromium/Electron renderer smoke test for GitHub-hosted Windows.
 * Test-only entrypoint, not shipped with the app. All disk information is
 * static mock data; no raw disk/image APIs are invoked or elevated.
 */
const { app, BrowserWindow, ipcMain, nativeTheme, screen } = require("electron");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const projectRoot = path.resolve(__dirname, "..");
const output = path.join(projectRoot, "ui-artifacts");
fs.mkdirSync(output, { recursive: true });
const report = { platform: process.platform, screenshots: [], skippedScreenshots: [], assertions: [], errors: [] };
let windowRef;
let inventoryRequests = 0;
let imageOpenRequests = 0;
let imageExportRequests = 0;
const watchdog = setTimeout(() => fail(new Error("Windows UI smoke timed out after 90 seconds")), 90_000);

function check(condition, message) {
  assert.ok(condition, message);
  report.assertions.push(message);
}
function fail(error) {
  const message = error instanceof Error ? error.stack || error.message : String(error);
  report.errors.push(message);
  console.error(message);
  try {
    fs.writeFileSync(path.join(output, "ui-smoke-report.json"), JSON.stringify(report, null, 2));
  } finally {
    clearTimeout(watchdog);
    app.exit(1);
  }
}
async function js(source) {
  return windowRef.webContents.executeJavaScript(source, true);
}
async function waitUntil(source, timeoutMs = 12_000) {
  const since = Date.now();
  while (Date.now() - since < timeoutMs) {
    try {
      if (await js(source)) return;
    } catch {
      // The page may not yet have executed scripts.
    }
    await new Promise(resolve => setTimeout(resolve, 120));
  }
  throw new Error("Renderer did not reach expected state: " + source);
}
async function snap(name) {
  if (process.env.PARTITION11_UI_SMOKE_SKIP_CAPTURE === "1") {
    report.skippedScreenshots.push(name);
    return;
  }
  await new Promise(resolve => setTimeout(resolve, 220));
  const screenshot = await windowRef.webContents.capturePage();
  const fileName = name + ".png";
  fs.writeFileSync(path.join(output, fileName), screenshot.toPNG());
  report.screenshots.push(fileName);
  check(screenshot.getSize().width > 500, "Screenshot captured: " + fileName);
}
function bridge() {
  ipcMain.handle("partition11:status", () => ({
    protocolVersion: 1,
    appVersion: "0.1.0",
    phase: "P0",
    agentConnected: false,
    capabilities: {
      inspectImages: false,
      inspectPhysicalDisks: true,
      writeImages: false,
      writePhysicalDisks: false
    }
  }));
  ipcMain.handle("partition11:display-info", () => {
    const display = screen.getDisplayMatching(windowRef.getBounds());
    return {
      protocolVersion: 1,
      scaleFactor: display.scaleFactor,
      workAreaWidth: display.workArea.width,
      workAreaHeight: display.workArea.height,
      highContrast: nativeTheme.shouldUseHighContrastColors,
      platform: process.platform === "win32" ? "windows" : "other"
    };
  });
  ipcMain.handle("partition11:benchmark-availability", () => ({
    protocolVersion: 1,
    phase: "P2.5A",
    mode: "preview",
    engineAvailable: false,
    canRead: false,
    canWrite: false
  }));
  ipcMain.handle("partition11:read-only-disk-inventory", () => {
    inventoryRequests += 1;
    return {
      protocolVersion: 2,
      status: "ok",
      scannedAt: "2026-10-19T12:00:00.000Z",
      disks: [{
        ordinal: 1,
        availability: "ready",
        capacityBytes: "68719476736",
        logicalSectorSize: 512,
        physicalSectorSize: 4096,
        busType: "usb",
        partitionStyle: "gpt",
        partitionCount: 2
      }],
      volumes: [{
        ordinal: 1,
        mountPoint: "E:\\",
        driveType: "removable",
        fileSystem: "exFAT",
        totalBytes: "64000000000",
        availableBytes: "32000000000",
        readOnly: false,
        eligible: false,
        eligibilityReason: "system_volume_unknown",
        diskOrdinal: null,
        partitionNumber: null,
        selectionToken: null
      }]
    };
  });
  ipcMain.handle("partition11:image-open", () => {
    imageOpenRequests += 1;
    return {
      status: "opened",
      sessionId: "smoke-session",
      imageName: "fixture.ext4",
      directoryId: "smoke-root",
      entries: [
        { id: "smoke-folder", name: "docs", kind: "directory", sizeBytes: "0" },
        { id: "smoke-file", name: "hello.txt", kind: "file", sizeBytes: "5" }
      ],
      nextPageToken: null
    };
  });
  ipcMain.handle("partition11:image-list-directory", (_event, sessionId, directoryId) => {
    if (sessionId !== "smoke-session" || directoryId !== "smoke-folder") {
      return { status: "error", error: "session_expired" };
    }
    return {
      status: "ok",
      entries: [{ id: "smoke-nested-file", name: "nested.txt", kind: "file", sizeBytes: "10" }],
      nextPageToken: null
    };
  });
  ipcMain.handle("partition11:image-next-page", () => ({ status: "error", error: "session_expired" }));
  ipcMain.handle("partition11:image-export-file", (_event, sessionId, fileId) => {
    if (sessionId !== "smoke-session" || fileId !== "smoke-file") {
      return { status: "error", error: "session_expired" };
    }
    imageExportRequests += 1;
    return { status: "exported", bytes: "5" };
  });
  ipcMain.handle("partition11:image-close-session", () => undefined);  ipcMain.handle("partition11:titlebar-theme", (_, theme) => {
    if (!["light", "dark"].includes(theme)) throw new Error("Invalid theme");
    windowRef.setTitleBarOverlay({
      color: theme === "dark" ? "#1b2028" : "#eef2f7",
      symbolColor: theme === "dark" ? "#f0f3fa" : "#202938",
      height: 80
    });
  });
}
async function main() {
  nativeTheme.themeSource = "light";
  bridge();
  windowRef = new BrowserWindow({
    width: 1460,
    height: 980,
    show: true,
    titleBarStyle: "hidden",
    titleBarOverlay: { color: "#eef2f7", symbolColor: "#202938", height: 80 },
    backgroundColor: "#f7f8fa",
    webPreferences: {
      partition: "windows-ui-smoke",
      preload: path.join(projectRoot, "dist", "main", "main", "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true
    }
  });
  windowRef.webContents.on("console-message", details => {
    if (details.level === "error") console.error("Renderer:", details.message);
  });
  await windowRef.loadFile(path.join(projectRoot, "dist", "renderer", "index.html"));
  await waitUntil("Boolean(document.querySelector('[data-testid=\"inventory-idle\"]'))");
  await waitUntil("document.querySelector('.status-left')?.textContent?.includes('Hazır')");
  check((await js("document.querySelector('h1')?.textContent")) === "Disk envanteri", "Read-only disk inventory is the default view");
  check(inventoryRequests === 0, "Opening the app never starts a physical disk query");
  check(await js("document.querySelector('.inventory-safety-note')?.textContent.includes('kapalı')"), "Inventory view states all disabled operations");
  await snap("01-inventory-idle");
  await js("document.querySelector('.inventory-refresh').click()");
  await waitUntil("Boolean(document.querySelector('[data-testid=\"inventory-disk-1\"]'))");
  check(inventoryRequests === 1, "Only an explicit refresh requests disk inventory");
  check(await js("document.querySelectorAll('.inventory-row button').length === 0"), "Real inventory rows are not selectable controls");
  check(await js("document.querySelector('[data-testid=\"inventory-disk-1\"]')?.textContent.includes('USB')"), "Inventory displays the mock USB bus independently");
  check(await js("document.querySelector('.inventory-row')?.textContent.includes('64.0 GiB')"), "Disk capacity is rendered from the allowlisted metadata");
  check(await js("document.querySelector('[data-testid=\"inventory-volume-1\"]')?.textContent.includes('E:\\\\')"), "Sanitized mock volume metadata is rendered");
  check(await js("document.querySelector('[data-testid=\"inventory-volume-1\"]')?.textContent.includes('Windows sistem diski doğrulanamadı')"), "Unverified system-disk mapping stays ineligible");
  await snap("02-readonly-inventory");
  await js("document.querySelector('.view-switch button:nth-child(3)').click()");
  await waitUntil("document.querySelector('h1')?.textContent === 'EXT4 imajlarını incele'");
  check(imageOpenRequests === 0, "Opening the image view does not start image file access");
  await js("document.querySelector('.image-card-heading .inventory-refresh').click()");
  await waitUntil("Boolean(document.querySelector('.image-entry-list'))");
  check(imageOpenRequests === 1, "Only an explicit image-open action requests an image");
  check(await js("!document.body.textContent.includes('D:\\\\\\\\private')"), "Image host paths are not rendered");
  await js("document.querySelector('.image-entry-name').click()");
  await waitUntil("document.querySelector('.image-breadcrumbs')?.textContent.includes('docs')");
  check(await js("document.querySelector('.image-entry-list')?.textContent.includes('nested.txt')"), "Image browser lists a selected directory through an opaque token");
  await js("document.querySelector('.image-breadcrumbs button').click()");
  await waitUntil("document.querySelector('.image-entry-list')?.textContent.includes('hello.txt')");
  await js("document.querySelector('.image-export-button').click()");
  await waitUntil("document.querySelector('.image-message[role=status]')?.textContent.includes('dışa aktarıldı')");
  check(imageExportRequests === 1, "Image file export uses the main-process save dialog bridge");
  await snap("03-image-browser");
  windowRef.setSize(520, 780);
  await new Promise(resolve => setTimeout(resolve, 350));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Image browser has no horizontal overflow at narrow width");
  windowRef.setSize(1460, 980);

  await js("document.querySelector('.view-switch button:nth-child(2)').click()");
  await waitUntil("Boolean(document.querySelector('[data-testid=\"format-workspace\"]'))");
  check((await js("document.querySelector('h1')?.textContent")) === "Biçimlendir", "Synthetic formatter demo remains separate from real inventory");
  check((await js("document.querySelectorAll('.disk-item').length")) === 3, "Three synthetic disk cards are rendered");
  check(await js("document.querySelector('[data-testid=\"disk-sample-system\"]').disabled"), "Protected system disk is disabled");
  check(await js("document.querySelector('[data-testid=\"format-button\"]').disabled"), "Formatting is strictly disabled");
  check((await js("document.body.textContent.includes('Örnek veriler')")), "Synthetic disk data is identified in UI");
  check(!(await js("Boolean(document.querySelector('.field-error'))")), "Default label passes validation");
  check((await js("document.querySelector('.partition-theme')?.dataset.theme")) === "light", "Light color scheme renders initially");

  await snap("04-light-demo");
  await js("document.querySelector('[data-testid=\"disk-sample-sandisk\"]').click()");
  await waitUntil("document.querySelector('#selected-disk-heading')?.textContent === 'SanDisk Ultra'");
  check((await js("document.querySelector('.partition-head')?.textContent")).includes("FAT32"), "Selecting a sample disk updates the partition summary");
  await js("document.querySelector('[data-testid=\"advanced-settings-toggle\"]').click()");
  await waitUntil("document.getElementById('advanced-settings') !== null");
  check((await js("document.querySelector('[data-testid=\"advanced-settings-toggle\"]').getAttribute('aria-expanded')")) === "true", "Advanced controls announce expanded state");
  await snap("05-advanced-sandisk");

  await js("document.querySelector('.theme-toggle').click()");
  await waitUntil("document.querySelector('.partition-theme')?.dataset.theme === 'dark'");
  check(await js("getComputedStyle(document.querySelector('.partition-theme')).colorScheme === 'dark'"), "Dark theme has matching browser color scheme");
  await snap("06-dark-reference");

  windowRef.setSize(860, 750);
  await new Promise(resolve => setTimeout(resolve, 400));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Compact desktop layout has no horizontal overflow");
  await snap("07-narrow-desktop");

  windowRef.setSize(520, 780);
  await new Promise(resolve => setTimeout(resolve, 400));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Small-screen responsive layout has no horizontal overflow");
  check(await js("document.querySelector('[data-testid=\"format-button\"]').disabled"), "Format remains disabled at narrow size");
  await snap("07-narrow-layout");

  // The hosted Windows display can be only 1024×768. Capture the lower
  // warning and action area separately so the entire reference can be
  // reviewed without needing a larger physical monitor.
  windowRef.setSize(1024, 768);
  await new Promise(resolve => setTimeout(resolve, 350));
  await js("document.querySelector('.warning-box').scrollIntoView({block: 'start'})");
  check(await js("document.querySelector('.warning-box')?.textContent.includes('tüm dosyaları siler')"), "Destructive-action warning is visible");
  check(await js("document.querySelector('[data-testid=format-button]').disabled"), "Format is still disabled in the bottom action area");
  await snap("08-warning-and-action");

  // P2.5A: benchmark screen must be reachable yet never write data.
  await js("document.querySelector('[data-testid=screen-tab-benchmark]').click()");
  await waitUntil("document.querySelector('h1')?.textContent === 'Disk Benchmark'");
  check(await js("document.querySelectorAll('[data-testid^=benchmark-profile-]').length === 4"), "Four benchmark workload presets are visible");
  check(await js("document.querySelectorAll('.bench-value').length === 8"), "Separate read/write metrics exist for all profiles");
  check(await js("Array.from(document.querySelectorAll('.bench-value')).every(cell => cell.textContent.includes('—'))"), "Every unmeasured score stays blank");
  check(await js("document.querySelector('[data-testid=benchmark-start]').disabled"), "Benchmark Start is disabled in P2.5A");
  check(await js("document.querySelector('[data-testid=benchmark-cancel]').disabled"), "Benchmark Cancel is disabled in P2.5A");
  check(await js("document.querySelector('[data-testid=benchmark-impact-estimate]').textContent.includes('3,5 GB') && document.querySelector('[data-testid=benchmark-impact-estimate]').textContent.includes('35 saniye')"), "Write impact disclosure includes the one-repeat byte and time bounds");
  check(await js("!document.querySelector('.benchmark-settings select') && document.querySelector('[data-testid=benchmark-repetitions]')?.textContent === '1'"), "A consent covers exactly one fixed benchmark run");
  check(await js("document.body.textContent.includes('Örnek veri')"), "Benchmark target is marked synthetic");
  await js("document.querySelector('.theme-toggle').click()");
  await waitUntil("document.querySelector('.partition-theme')?.dataset.theme === 'light'");
  await snap("09-benchmark-light");

  await js("document.querySelector('[data-testid=screen-tab-benchmark]') && document.querySelector('.bench-details-toggle').click()");
  await waitUntil("document.getElementById('benchmark-details') !== null");
  check(await js("document.querySelector('.bench-details-toggle').getAttribute('aria-expanded') === 'true'"), "Benchmark metrics details expand accessibly");
  await js("document.querySelector('[data-testid=disk-sample-samsung]').click()");
  await waitUntil("document.querySelector('#benchmark-target-title')?.textContent === 'Samsung T7'");
  check(await js("document.querySelector('[data-testid=benchmark-start]').disabled"), "Changing targets never enables live benchmarking");
  await js("document.querySelector('.theme-toggle').click()");
  await waitUntil("document.querySelector('.partition-theme')?.dataset.theme === 'dark'");
  await snap("10-benchmark-dark");

  windowRef.setSize(520, 780);
  await new Promise(resolve => setTimeout(resolve, 360));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Narrow benchmark page has no document-wide horizontal overflow");
  await snap("11-benchmark-narrow");

  await js("document.querySelector('[data-testid=screen-tab-format]').click()");
  await waitUntil("document.querySelector('h1')?.textContent === 'Biçimlendir'");
  check(await js("document.querySelector('[data-testid=format-button]').disabled"), "Returning to format screen keeps format action disabled");

  fs.writeFileSync(path.join(output, "ui-smoke-report.json"), JSON.stringify(report, null, 2));
  clearTimeout(watchdog);
  windowRef.close();
  app.exit(0);
}
app.on("window-all-closed", () => {});
app.whenReady().then(main).catch(fail);
