import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

test("production renderer CSP prohibits remote and loopback access", () => {
  const html = readFileSync(path.resolve("dist/renderer/index.html"), "utf8");
  assert.match(html, /connect-src 'self';/);
  assert.match(html, /script-src 'self';/);
  assert.doesNotMatch(html, /ws:\/\/127\.0\.0\.1/);
  assert.doesNotMatch(html, /http:\/\/127\.0\.0\.1/);
  assert.doesNotMatch(html, /script-src 'self' 'unsafe-inline'/);
});
