import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, lstat, readFile, rename, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import type { BenchmarkReport, WorkerBenchmarkReport } from "../shared/benchmark";
import { buildDiskSpdFileArgs, validateWorkload } from "./benchmarkPolicy";
import { OwnedScratchLease } from "./benchmarkScratch";
import {
  BenchmarkRunController, buildBenchmarkAuthorizationEnvelope,
  validateBenchmarkStartRequest
} from "./benchmarkRunController";

const read = { profileId: "seq1m-q8t1", direction: "read",
               warmupSeconds: 0, measurementSeconds: 2, repetitions: 1 } as const;
const write = { ...read, profileId: "rnd4k-q32t1", direction: "write" } as const;

test("DiskSpd presets generate fixed file-only flags with no shell", () => {
  const file = "C:\\p11-ci\\scratch.bin";
  const r = buildDiskSpdFileArgs(file, read);
  assert.deepEqual(r.args, ["-b1M", "-o8", "-t1", "-w0", "-W0", "-d2", "-L", "-Rxml", file]);
  assert.equal(r.measurementSeconds, read.measurementSeconds);
  assert.equal(r.expectedWrite, false);
  assert.equal(r.estimatedMaxWriteBytes, 0);
  const w = buildDiskSpdFileArgs(file, write);
  assert.deepEqual(w.args, ["-b4K", "-o32", "-t1", "-w100", "-W0", "-d2", "-g100000", "-L", "-Rxml", "-r4K", file]);
  assert.equal(w.expectedWrite, true);
  assert.equal(w.estimatedMaxWriteBytes, 1_700_000_000);
  assert.ok(w.maxWallClockSeconds < 60);
});

test("write estimate includes the full bounded process deadline for exactly one run", () => {
  const plan = buildDiskSpdFileArgs("D:\\p11-ci\\scratch.bin", {
    ...write, warmupSeconds: 5, measurementSeconds: 15, repetitions: 1
  });
  assert.equal(plan.maxWallClockSeconds, 35);
  assert.equal(plan.estimatedMaxWriteBytes, 3_500_000_000);
});

test("largest allowed write plan has a finite conservative byte budget", () => {
  const plan = buildDiskSpdFileArgs("D:\\p11-ci\\scratch.bin", {
    ...write, warmupSeconds: 15, measurementSeconds: 60, repetitions: 1
  });
  assert.equal(plan.maxWallClockSeconds, 90);
  assert.equal(plan.estimatedMaxWriteBytes, 9_000_000_000);
  assert.ok(Number.isSafeInteger(plan.estimatedMaxWriteBytes));
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
    { ...read, warmupSeconds: -1 }, { ...read, repetitions: 2 },
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

const startRequest = (overrides: Record<string, unknown> = {}) => ({
  requestId: "8c6f35f8-260e-4fbd-9e4d-c91b78562f58",
  selectionToken: "c7a21949-9d22-4a41-b165-5c00c021179e",
  profileId: "seq1m-q8t1",
  direction: "read",
  ...overrides
});

function consumedSelection() {
  return {
    selection: {
      volumeOrdinal: 1,
      mountPoint: "E:\\",
      fileSystem: "NTFS",
      totalBytes: "10737418240",
      availableBytes: "4294967296",
      diskOrdinal: 2,
      partitionNumber: 3,
      identityHash: "22".repeat(32)
    },
    identityKey: Buffer.from("11".repeat(32), "hex")
  } as never;
}

function validBenchmarkReport(
  overrides: Partial<Omit<WorkerBenchmarkReport, "targetVolumeId">> & { targetVolumeId?: string } = {}
): WorkerBenchmarkReport {
  const base: WorkerBenchmarkReport = {
    protocolVersion: 1,
    source: "measured",
    engine: "DiskSpd",
    engineVersion: "2.3.0",
    filesystem: "NTFS",
    targetVolumeId: "22".repeat(32) as WorkerBenchmarkReport["targetVolumeId"],
    cachePolicy: "unbuffered",
    testFileBytes: 1024 * 1024 * 1024,
    warmupSeconds: 5,
    measurementSeconds: 15,
    measuredAtUtc: "2026-10-20T10:00:00.000Z",
    measurements: [{
      profileId: "seq1m-q8t1",
      direction: "read",
      throughputMBps: 100,
      iops: 100,
      latencyAvgMs: 0.5,
      latencyP95Ms: 0.8,
      latencyP99Ms: 1.2
    }]
  };
  return { ...base, ...overrides } as WorkerBenchmarkReport;
}

test("main accepts only a sanitized worker report bound to this consented run", async () => {
  const measurement = validBenchmarkReport().measurements[0]!;
  const invalidReports = [
    validBenchmarkReport({ targetVolumeId: "33".repeat(32) }),
    validBenchmarkReport({ filesystem: "exFAT" }),
    validBenchmarkReport({ engineVersion: "9.9.9" }),
    validBenchmarkReport({ cachePolicy: "buffered" }),
    validBenchmarkReport({ testFileBytes: 4 * 1024 * 1024 }),
    validBenchmarkReport({ warmupSeconds: 0 }),
    validBenchmarkReport({ measurementSeconds: 14 }),
    validBenchmarkReport({ measuredAtUtc: "yesterday" }),
    validBenchmarkReport({ measurements: [{ ...measurement, profileId: "rnd4k-q32t1" }] }),
    validBenchmarkReport({ measurements: [{ ...measurement, direction: "write" }] }),
    validBenchmarkReport({ measurements: [measurement, measurement] }),
    { ...validBenchmarkReport(), privatePath: "E:\\private.bin" },
    validBenchmarkReport({ measurements: [{ ...measurement, privatePath: "E:\\private.bin" } as never] }),
    validBenchmarkReport({ measurements: [{ ...measurement, throughputMBps: Number.NaN }] })
  ];

  for (const report of invalidReports) {
    const controller = new BenchmarkRunController({
      runtimeAvailable: () => true,
      consumeSelection: () => consumedSelection(),
      confirm: async () => true,
      execute: async () => ({ status: "completed", report } as never)
    });
    assert.deepEqual(await controller.start(startRequest()),
      { status: "error", error: "invalid_response" });
  }

  const validController = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => consumedSelection(),
    confirm: async () => true,
    execute: async () => ({ status: "completed", report: validBenchmarkReport() })
  });
  const result = await validController.start(startRequest());
  assert.deepEqual(result, {
    status: "completed",
    report: { ...validBenchmarkReport(), targetVolumeId: startRequest().selectionToken }
  });
  assert.equal(result.status === "completed" && result.report.targetVolumeId, startRequest().selectionToken);
});

test("write reports require write-through policy and expose only the one-use target token", async () => {
  const request = startRequest({
    requestId: "9c6f35f8-2600-4fbd-9e4d-c91b78562f58",
    selectionToken: "8c6f35f8-2600-4fbd-9e4d-c91b78562f58",
    direction: "write"
  });
  const report: WorkerBenchmarkReport = {
    ...validBenchmarkReport(),
    cachePolicy: "write-through",
    measurements: [{ ...validBenchmarkReport().measurements[0]!, direction: "write" }]
  };
  let returned: unknown;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => consumedSelection(),
    confirm: async () => true,
    execute: async () => ({ status: "completed", report }),
    createAuthorizationKey: () => Buffer.from("33".repeat(32), "hex")
  });
  returned = await controller.start(request);
  assert.deepEqual(returned, {
    status: "completed",
    report: {
      ...report,
      targetVolumeId: request.selectionToken,
      measurements: [{ ...report.measurements[0] }]
    }
  });
  assert.equal(JSON.stringify(returned).includes("22".repeat(32)), false);
});

test("unknown worker result fields fail closed and latch cleanup-required", async () => {
  let consumed = 0;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => { consumed += 1; return consumedSelection(); },
    confirm: async () => true,
    execute: async () => ({ status: "cancelled", localPath: "E:\\scratch.bin" } as never)
  });
  assert.deepEqual(await controller.start(startRequest()),
    { status: "error", error: "invalid_response" });
  assert.deepEqual(await controller.start(startRequest({ requestId: "9c6f35f8-2600-4fbd-9e4d-c91b78562f58" })),
    { status: "error", error: "cleanup_required" });
  assert.equal(consumed, 1);
});

test("malformed successful worker output latches cleanup-required and blocks another run", async () => {
  let consumed = 0;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => { consumed += 1; return consumedSelection(); },
    confirm: async () => true,
    execute: async () => ({
      status: "completed",
      report: validBenchmarkReport({ targetVolumeId: "33".repeat(32) })
    })
  });

  assert.deepEqual(await controller.start(startRequest()),
    { status: "error", error: "invalid_response" });
  assert.deepEqual(await controller.start(startRequest({ requestId: "9c6f35f8-260e-4fbd-9e4d-c91b78562f58" })),
    { status: "error", error: "cleanup_required" });
  assert.equal(consumed, 1);
});

test("benchmark start accepts only a token, allowlisted profile and direction", () => {
  assert.deepEqual(validateBenchmarkStartRequest(startRequest()), startRequest());
  for (const invalid of [
    startRequest({ path: "E:\\important.bin" }),
    startRequest({ executable: "diskspd.exe" }),
    startRequest({ args: ["-w100"] }),
    startRequest({ durationSeconds: 3600 }),
    startRequest({ repetitions: 8 }),
    startRequest({ profileId: "anything" }),
    startRequest({ direction: "erase" }),
    startRequest({ requestId: "stale" }),
    { requestId: startRequest().requestId, selectionToken: startRequest().selectionToken, profileId: "seq1m-q8t1" }
  ]) assert.throws(() => validateBenchmarkStartRequest(invalid));
});

test("agent authorization contains no renderer path and binds only one fixed run", () => {
  const envelope = buildBenchmarkAuthorizationEnvelope(
    validateBenchmarkStartRequest(startRequest()),
    consumedSelection(),
    Buffer.from("33".repeat(32), "hex")
  );
  assert.deepEqual(Object.keys(envelope).sort(), [
    "authorizationKey", "authorizationTag", "direction", "diskOrdinal", "identityHash",
    "identityKey", "operation", "partitionNumber", "profileId", "protocolVersion",
    "requestId", "volumeOrdinal"
  ].sort());
  assert.equal(envelope.operation, "runFileBenchmark");
  assert.equal(envelope.volumeOrdinal, 1);
  assert.equal(envelope.diskOrdinal, 2);
  assert.equal(envelope.partitionNumber, 3);
  assert.equal(envelope.identityKey, "11".repeat(32));
  assert.equal(envelope.authorizationKey, "33".repeat(32));
  assert.equal(envelope.authorizationTag, "ce971eb08282bd2a3d4c527184083fc77abe86fa5e218930f7cc2dab5ac3d5d5");
  assert.equal("selectionToken" in envelope, false);
  assert.equal("mountPoint" in envelope, false);
  assert.equal("durationSeconds" in envelope, false);
  assert.equal("args" in envelope, false);
});

test("disabled runtime fails closed before consuming the selection or showing consent", async () => {
  let consumed = 0;
  let consented = 0;
  let executed = 0;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => false,
    consumeSelection: () => { consumed += 1; return consumedSelection(); },
    confirm: async () => { consented += 1; return true; },
    execute: async () => { executed += 1; return { status: "error", error: "worker_failure" }; }
  });
  assert.deepEqual(await controller.start(startRequest()), { status: "unavailable" });
  assert.deepEqual([consumed, consented, executed], [0, 0, 0]);
});

test("consent denial consumes the one-use selection but never starts the worker", async () => {
  let consumed = 0;
  let executed = 0;
  let details: unknown;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => { consumed += 1; return consumedSelection(); },
    confirm: async summary => { details = summary; return false; },
    execute: async () => { executed += 1; return { status: "error", error: "worker_failure" }; }
  });
  assert.deepEqual(await controller.start(startRequest({ direction: "write" })), { status: "consent_declined" });
  assert.equal(consumed, 1);
  assert.equal(executed, 0);
  assert.deepEqual(await controller.start(startRequest()), { status: "stale_request" });
  assert.equal(consumed, 1);
  assert.deepEqual(details, {
    mountPoint: "E:\\", fileSystem: "NTFS", profileId: "seq1m-q8t1", direction: "write",
    scratchFileBytes: 1024 * 1024 * 1024, warmupSeconds: 5, measurementSeconds: 15,
    maxDurationSeconds: 35, maxWriteBytes: 3_500_000_000
  });
});

test("one active consent blocks a second run and cancel during consent prevents launch", async () => {
  let releaseConsent!: (approved: boolean) => void;
  let executeCount = 0;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => consumedSelection(),
    confirm: () => new Promise<boolean>(resolve => { releaseConsent = resolve; }),
    execute: async () => { executeCount += 1; return { status: "error", error: "worker_failure" }; }
  });
  const first = controller.start(startRequest());
  await Promise.resolve();
  assert.deepEqual(await controller.start(startRequest({ requestId: "9c6f35f8-260e-4fbd-9e4d-c91b78562f58" })), { status: "busy" });
  assert.deepEqual(controller.cancel("00000000-0000-4000-8000-000000000000"), { status: "stale_cancel" });
  assert.deepEqual(controller.cancel(startRequest().requestId), { status: "cancelling" });
  releaseConsent(true);
  assert.deepEqual(await first, { status: "cancelled" });
  assert.equal(executeCount, 0);
});

test("cancel during execution stays busy until the worker confirms drain and cleanup", async () => {
  let releaseWorker!: (result: { status: "cancelled" } | { status: "error"; error: "cleanup_required" }) => void;
  let workerStarted = false;
  let workerSignal: AbortSignal | undefined;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => consumedSelection(),
    confirm: async () => true,
    execute: async (_request, signal) => {
      workerStarted = true;
      workerSignal = signal;
      return new Promise(resolve => { releaseWorker = resolve; });
    }
  });
  const first = controller.start(startRequest());
  await Promise.resolve();
  assert.equal(workerStarted, true);
  assert.equal(workerSignal?.aborted, false);
  assert.deepEqual(controller.cancel(startRequest().requestId), { status: "cancelling" });
  assert.equal(workerSignal?.aborted, true);
  assert.deepEqual(await controller.start(startRequest({ requestId: "9c6f35f8-260e-4fbd-9e4d-c91b78562f58" })), { status: "busy" });
  releaseWorker({ status: "cancelled" });
  assert.deepEqual(await first, { status: "cancelled" });
});

test("cancellation never returns success if the worker has not proven cleanup", async () => {
  let releaseWorker!: (result: { status: "completed"; report: never }) => void;
  const controller = new BenchmarkRunController({
    runtimeAvailable: () => true,
    consumeSelection: () => consumedSelection(),
    confirm: async () => true,
    execute: async () => new Promise(resolve => { releaseWorker = resolve; })
  });
  const first = controller.start(startRequest());
  await Promise.resolve();
  controller.cancel(startRequest().requestId);
  releaseWorker({ status: "completed", report: {} as never });
  assert.deepEqual(await first, { status: "error", error: "cleanup_required" });
});
