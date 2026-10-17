import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, lstat, readFile, rename, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { buildDiskSpdFileArgs, validateWorkload } from "./benchmarkPolicy";
import { OwnedScratchLease } from "./benchmarkScratch";

const read = { profileId: "seq1m-q8t1", direction: "read",
               warmupSeconds: 0, measurementSeconds: 2, repetitions: 1 } as const;
const write = { ...read, profileId: "rnd4k-q32t1", direction: "write" } as const;

test("DiskSpd presets generate fixed file-only flags with no shell", () => {
  const file = "C:\\p11-ci\\scratch.bin";
  const r = buildDiskSpdFileArgs(file, read);
  assert.deepEqual(r.args, ["-b1M", "-o8", "-t1", "-w0", "-W0", "-d2", "-L", "-Rxml", file]);
  assert.equal(r.expectedWrite, false);
  const w = buildDiskSpdFileArgs(file, write);
  assert.deepEqual(w.args, ["-b4K", "-o32", "-t1", "-w100", "-W0", "-d2", "-L", "-Rxml", "-r4K", file]);
  assert.equal(w.expectedWrite, true);
  assert.ok(w.maxWallClockSeconds < 60);
});

test("rejects target specifiers, device paths, root paths, injections", () => {
  for (const file of [
    "", "C:", "C:\\", "\\\\?\\C:\\file", "\\\\.\\PhysicalDrive0",
    "\\\\server\\share\\file", "C:\\a\\..\\b", "C:\\a\\file\n-r4K",
    "C:\\a\\file\0", "C:\\a\\file*"
  ]) {
    assert.throws(() => buildDiskSpdFileArgs(file, read), file);
  }
});
test("rejects arbitrary fields and unsafe numeric workload bounds", () => {
  for (const item of [
    { ...read, path: "C:\\user\\important" },
    { ...read, direction: "erase" }, { ...read, profileId: "invalid" },
    { ...read, measurementSeconds: Number.NaN }, { ...read, measurementSeconds: 3600 },
    { ...read, warmupSeconds: -1 }, { ...read, repetitions: 9 },
    { ...read, repetitions: 1.5 }, { ...read, measurementSeconds: 0 }
  ]) assert.throws(() => validateWorkload(item));
});

test("lease creates only a new, private aligned scratch file and removes exact file", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "p11-test-root-"));
  try {
    const canonical = await import("node:fs/promises").then(fs => fs.realpath(root));
    const lease = await OwnedScratchLease.createInsideTrustedTestRoot(canonical, 4 * 1024 * 1024);
    assert.equal((await lstat(lease.filePath)).size, 4 * 1024 * 1024);
    await lease.verify();
    await lease.cleanup();
    await lease.cleanup();
    await assert.rejects(lease.verify());
    await assert.rejects(lstat(lease.filePath));
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("cleanup refuses a replaced file and never deletes the foreign replacement", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "p11-test-root-"));
  const canonical = await import("node:fs/promises").then(fs => fs.realpath(root));
  try {
    const lease = await OwnedScratchLease.createInsideTrustedTestRoot(canonical, 4 * 1024 * 1024);
    const old = lease.filePath + ".old";
    await rename(lease.filePath, old);
    await writeFile(lease.filePath, "foreign user data", { flag: "wx" });
    await assert.rejects(lease.cleanup(), /identity changed/);
    assert.equal(await readFile(lease.filePath, "utf8"), "foreign user data");
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("scratch sizes must remain CI-only and filesystem-block aligned", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "p11-test-root-"));
  const canonical = await import("node:fs/promises").then(fs => fs.realpath(root));
  try {
    for (const bytes of [0, 1, 4000, 3 * 1024 * 1024, 65 * 1024 * 1024, Number.NaN]) {
      await assert.rejects(OwnedScratchLease.createInsideTrustedTestRoot(canonical, bytes));
    }
  } finally { await rm(root, { recursive: true, force: true }); }
});
