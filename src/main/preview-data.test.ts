import assert from "node:assert/strict";
import { test } from "node:test";
import { sampleDisks, validateExtLabel } from "../renderer/previewData";

test("sample data is explicitly synthetic with protected system disk", () => {
  assert.equal(sampleDisks.length, 3);
  assert.equal(sampleDisks.filter(d => d.protected).length, 1);
  assert.equal(sampleDisks.filter(d => !d.protected).length, 2);
  assert.ok(sampleDisks.every(d => d.id.startsWith("sample-")));
  assert.equal(sampleDisks[0].partition?.filesystem, "NTFS");
});

test("EXT4 volume labels are bounded by UTF-8 bytes", () => {
  assert.equal(validateExtLabel("LinuxData").valid, true);
  assert.equal(validateExtLabel("1234567890123456").valid, true);
  assert.equal(validateExtLabel("12345678901234567").valid, false);
  assert.equal(validateExtLabel("ğ".repeat(9)).valid, false);
  assert.equal(validateExtLabel("a/b").valid, false);
});
