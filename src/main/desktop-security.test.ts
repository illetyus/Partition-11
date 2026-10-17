import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

// Static guardrail for high-risk Electron main-process settings. This is NOT
// a substitute for running a packaged Electron app on real Windows hardware.
const source = readFileSync(path.resolve("src/main/main.ts"), "utf8");

test("default session refuses every permission request and check", () => {
  assert.match(source, /setPermissionRequestHandler\([\s\S]*?callback\(false\)/);
  assert.match(source, /setPermissionCheckHandler\(\(\) => false\)/);
});

test("every future webContents denies navigation, windows and webviews", () => {
  assert.match(source, /app\.on\("web-contents-created"/);
  assert.match(source, /setWindowOpenHandler\(\(\) => \(\{ action: "deny" \}\)\)/);
  assert.match(source, /will-navigate/);
  assert.match(source, /will-attach-webview/);
});

test("primary renderer remains isolated from Node and webview APIs", () => {
  for (const clause of [
    /sandbox:\s*true/, /contextIsolation:\s*true/, /nodeIntegration:\s*false/,
    /webviewTag:\s*false/, /webSecurity:\s*true/
  ]) assert.match(source, clause);
});
