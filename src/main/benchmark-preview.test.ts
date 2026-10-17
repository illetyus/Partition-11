import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import path from "node:path";
import {
  BENCHMARK_PROFILES, BENCHMARK_PREVIEW_AVAILABILITY, isFiniteMeasurement,
  type BenchmarkMeasurement
} from "../shared/benchmark";

test("four approved presets have bounded block sizes and queue depths", () => {
  assert.deepEqual(BENCHMARK_PROFILES.map(p => p.id), [
    "seq1m-q8t1", "seq1m-q1t1", "rnd4k-q32t1", "rnd4k-q1t1"
  ]);
  assert.ok(BENCHMARK_PROFILES.every(p => [4096, 1048576].includes(p.blockBytes)));
  assert.ok(BENCHMARK_PROFILES.every(p => [1, 8, 32].includes(p.queueDepth)));
  assert.ok(BENCHMARK_PROFILES.every(p => p.threads === 1));
  assert.equal(new Set(BENCHMARK_PROFILES.map(p => p.id)).size, 4);
});

test("benchmark preview has no active I/O capability", () => {
  assert.deepEqual(BENCHMARK_PREVIEW_AVAILABILITY, {
    protocolVersion: 1, phase: "P2.5A", mode: "preview",
    engineAvailable: false, canRead: false, canWrite: false
  });
});

test("normalized measurement guard rejects nonfinite or negative values", () => {
  const result: BenchmarkMeasurement = {
    profileId: "seq1m-q8t1", direction: "read",
    throughputMBps: 700, iops: 684, latencyAvgMs: 1,
    latencyP95Ms: 2, latencyP99Ms: null
  };
  assert.ok(isFiniteMeasurement(result));
  assert.equal(isFiniteMeasurement({ ...result, throughputMBps: Number.NaN }), false);
  assert.equal(isFiniteMeasurement({ ...result, iops: Number.POSITIVE_INFINITY }), false);
  assert.equal(isFiniteMeasurement({ ...result, latencyAvgMs: -1 }), false);
});

test("P2.5A UI contains no active benchmark operation", () => {
  const source = readFileSync(path.resolve("src/renderer/BenchmarkPanel.tsx"), "utf8");
  assert.match(source, /className="bench-primary" disabled/);
  assert.match(source, /data-testid="benchmark-cancel"/);
  const mainSource = readFileSync(path.resolve("src/main/main.ts"), "utf8");
  assert.match(mainSource, /BENCHMARK_PREVIEW_AVAILABILITY/);
  assert.doesNotMatch(mainSource, /IPC\.BENCHMARK_START/);
});
