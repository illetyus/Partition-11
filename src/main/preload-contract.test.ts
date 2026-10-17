import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { IPC } from "../shared/protocol";

const compiledPreload = readFileSync(path.join(__dirname, "preload.js"), "utf8");

test("sandbox preload requires no arbitrary local modules", () => {
  assert.doesNotMatch(compiledPreload, /require\(["']\.\.?\//);
  assert.doesNotMatch(compiledPreload, /require\(["']node:/);
  assert.match(compiledPreload, /require\(["']electron["']\)/);
});

test("bridge exposes exact supported channel names", () => {
  for (const name of Object.values(IPC)) {
    assert.ok(compiledPreload.includes(name), `Expected channel ${name} in isolated preload`);
  }
  assert.doesNotMatch(compiledPreload, /ipcRenderer\.send\s*\(/);
});
