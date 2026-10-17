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
test("all disk capabilities remain disabled at the IPC boundary", () => {
  const status = readFileSync(path.resolve("src/main/main.ts"), "utf8");
  assert.match(status, /writeImages: false/);
  assert.match(status, /writePhysicalDisks: false/);
  assert.match(status, /inspectPhysicalDisks: false/);
  assert.match(status, /theme !== "dark" && theme !== "light"/);
});
