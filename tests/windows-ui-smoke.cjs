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
const report = { platform: process.platform, screenshots: [], assertions: [], errors: [] };
let windowRef;
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
      inspectPhysicalDisks: false,
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
  ipcMain.handle("partition11:titlebar-theme", (_, theme) => {
    if (!["light", "dark"].includes(theme)) throw new Error("Invalid theme");
    windowRef.setTitleBarOverlay({
      color: theme === "dark" ? "#1b2028" : "#eef2f7",
      symbolColor: theme === "dark" ? "#f0f3fa" : "#202938",
      height: 80
    });
  });
}
async function main() {
  bridge();
  windowRef = new BrowserWindow({
    width: 1460,
    height: 980,
    show: true,
    titleBarStyle: "hidden",
    titleBarOverlay: { color: "#eef2f7", symbolColor: "#202938", height: 80 },
    backgroundColor: "#f7f8fa",
    webPreferences: {
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
  await waitUntil("Boolean(document.querySelector('[data-testid=\"format-workspace\"]'))");
  await waitUntil("document.querySelector('.status-left')?.textContent?.includes('Hazır')");
  check((await js("document.querySelector('h1')?.textContent")) === "Biçimlendir", "Reference formatter heading is present");
  check((await js("document.querySelectorAll('.disk-item').length")) === 3, "Three synthetic disk cards are rendered");
  check(await js("document.querySelector('[data-testid=\"disk-sample-system\"]').disabled"), "Protected system disk is disabled");
  check(await js("document.querySelector('[data-testid=\"format-button\"]').disabled"), "Formatting is strictly disabled");
  check((await js("document.body.textContent.includes('Örnek veriler')")), "Synthetic disk data is identified in UI");
  check(!(await js("Boolean(document.querySelector('.field-error'))")), "Default label passes validation");
  check((await js("document.querySelector('.partition-theme')?.dataset.theme")) === "light", "Light color scheme renders initially");

  await snap("01-light-reference");
  await js("document.querySelector('[data-testid=\"disk-sample-sandisk\"]').click()");
  await waitUntil("document.querySelector('#selected-disk-heading')?.textContent === 'SanDisk Ultra'");
  check((await js("document.querySelector('.partition-head')?.textContent")).includes("FAT32"), "Selecting a sample disk updates the partition summary");
  await js("document.querySelector('[data-testid=\"advanced-settings-toggle\"]').click()");
  await waitUntil("document.getElementById('advanced-settings') !== null");
  check((await js("document.querySelector('[data-testid=\"advanced-settings-toggle\"]').getAttribute('aria-expanded')")) === "true", "Advanced controls announce expanded state");
  await snap("02-advanced-sandisk");

  await js("document.querySelector('.theme-toggle').click()");
  await waitUntil("document.querySelector('.partition-theme')?.dataset.theme === 'dark'");
  check(await js("getComputedStyle(document.querySelector('.partition-theme')).colorScheme === 'dark'"), "Dark theme has matching browser color scheme");
  await snap("03-dark-reference");

  windowRef.setSize(860, 750);
  await new Promise(resolve => setTimeout(resolve, 400));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Compact desktop layout has no horizontal overflow");
  await snap("04-narrow-desktop");

  windowRef.setSize(520, 780);
  await new Promise(resolve => setTimeout(resolve, 400));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Small-screen responsive layout has no horizontal overflow");
  check(await js("document.querySelector('[data-testid=\"format-button\"]').disabled"), "Format remains disabled at narrow size");
  await snap("05-narrow-layout");

  // The hosted Windows display can be only 1024×768. Capture the lower
  // warning and action area separately so the entire reference can be
  // reviewed without needing a larger physical monitor.
  windowRef.setSize(1024, 768);
  await new Promise(resolve => setTimeout(resolve, 350));
  await js("document.querySelector('.warning-box').scrollIntoView({block: 'start'})");
  check(await js("document.querySelector('.warning-box')?.textContent.includes('tüm dosyaları siler')"), "Destructive-action warning is visible");
  check(await js("document.querySelector('[data-testid=format-button]').disabled"), "Format is still disabled in the bottom action area");
  await snap("06-warning-and-action");

  // P2.5A: benchmark screen must be reachable yet never write data.
  await js("document.querySelector('[data-testid=screen-tab-benchmark]').click()");
  await waitUntil("document.querySelector('h1')?.textContent === 'Disk Benchmark'");
  check(await js("document.querySelectorAll('[data-testid^=benchmark-profile-]').length === 4"), "Four benchmark workload presets are visible");
  check(await js("document.querySelectorAll('.bench-value').length === 8"), "Separate read/write metrics exist for all profiles");
  check(await js("Array.from(document.querySelectorAll('.bench-value')).every(cell => cell.textContent.includes('—'))"), "Every unmeasured score stays blank");
  check(await js("document.querySelector('[data-testid=benchmark-start]').disabled"), "Benchmark Start is disabled in P2.5A");
  check(await js("document.querySelector('[data-testid=benchmark-cancel]').disabled"), "Benchmark Cancel is disabled in P2.5A");
  check(await js("document.body.textContent.includes('Örnek veri')"), "Benchmark target is marked synthetic");
  await js("document.querySelector('.theme-toggle').click()");
  await waitUntil("document.querySelector('.partition-theme')?.dataset.theme === 'light'");
  await snap("07-benchmark-light");

  await js("document.querySelector('[data-testid=screen-tab-benchmark]') && document.querySelector('.bench-details-toggle').click()");
  await waitUntil("document.getElementById('benchmark-details') !== null");
  check(await js("document.querySelector('.bench-details-toggle').getAttribute('aria-expanded') === 'true'"), "Benchmark metrics details expand accessibly");
  await js("document.querySelector('[data-testid=disk-sample-samsung]').click()");
  await waitUntil("document.querySelector('#benchmark-target-title')?.textContent === 'Samsung T7'");
  check(await js("document.querySelector('[data-testid=benchmark-start]').disabled"), "Changing targets never enables live benchmarking");
  await js("document.querySelector('.theme-toggle').click()");
  await waitUntil("document.querySelector('.partition-theme')?.dataset.theme === 'dark'");
  await snap("08-benchmark-dark");

  windowRef.setSize(520, 780);
  await new Promise(resolve => setTimeout(resolve, 360));
  check(await js("document.documentElement.scrollWidth <= window.innerWidth + 1"), "Narrow benchmark page has no document-wide horizontal overflow");
  await snap("09-benchmark-narrow");

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
