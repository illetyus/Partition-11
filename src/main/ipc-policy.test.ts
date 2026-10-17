import assert from "node:assert/strict";
import { test } from "node:test";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { isAllowedFrame } from "./ipc-policy";

const trustedPath = path.resolve("dist/renderer/index.html");
const trustedUrl = pathToFileURL(trustedPath).toString();

test("rejects no frame and remote websites", () => {
  assert.equal(isAllowedFrame(undefined, trustedPath), false);
  assert.equal(isAllowedFrame("https://attacker.invalid/", trustedPath), false);
  assert.equal(isAllowedFrame("data:text/html,bad", trustedPath), false);
});

test("accepts exactly the packaged renderer file", () => {
  assert.equal(isAllowedFrame(trustedUrl, trustedPath), true);
  assert.equal(isAllowedFrame(trustedUrl + "#evil", trustedPath), false);
  assert.equal(isAllowedFrame(trustedUrl + "?mode=evil", trustedPath), false);
  assert.equal(isAllowedFrame(pathToFileURL(path.resolve("another.html")).toString(), trustedPath), false);
});

test("development origin is restricted to local loopback root without query", () => {
  const dev = "http://127.0.0.1:5173";
  assert.equal(isAllowedFrame("http://127.0.0.1:5173/", trustedPath, dev), true);
  assert.equal(isAllowedFrame("http://localhost:5173/", trustedPath, dev), false);
  assert.equal(isAllowedFrame("http://127.0.0.1:5174/", trustedPath, dev), false);
  assert.equal(isAllowedFrame("http://127.0.0.1:5173/evil", trustedPath, dev), false);
  assert.equal(isAllowedFrame("http://127.0.0.1:5173/?evil", trustedPath, dev), false);
});
