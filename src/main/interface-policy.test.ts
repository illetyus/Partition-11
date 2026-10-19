import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

test("preview UI cannot enable a real formatting operation", () => {
  const source = readFileSync(path.resolve("src/renderer/App.tsx"), "utf8");
  assert.match(source, /data-testid="format-button"[^>]*type="button" disabled/);
  assert.match(source, /Örnek veriler/);
  assert.match(source, /disabled=\{disk.protected\}/);
  assert.match(source, /data-theme=\{dark \? "dark" : "light"\}/);
});
test("physical inspection requires the fixed agent and all writes remain disabled", () => {
  const status = readFileSync(path.resolve("src/main/main.ts"), "utf8");
  assert.match(status, /writeImages: false/);
  assert.match(status, /writePhysicalDisks: false/);
  assert.match(status, /inspectPhysicalDisks: process\.platform === "win32" && existsSync\(agentExecutablePath\(\)\)/);
  assert.match(status, /agentConnected: false/);
  assert.match(status, /READ_ONLY_DISK_INVENTORY/);
  assert.match(status, /args\.length !== 0/);
  assert.match(status, /theme !== "dark" && theme !== "light"/);
});
