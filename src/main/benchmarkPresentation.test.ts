import assert from "node:assert/strict";
import { test } from "node:test";
import { displayedMeasurement } from "../renderer/benchmarkPresentation";
import type { BenchmarkReport } from "../shared/benchmark";

const report: BenchmarkReport = {
  protocolVersion: 1,
  source: "measured", engine: "DiskSpd", engineVersion: "2.3",
  filesystem: "NTFS", targetVolumeId: "trusted-volume-test",
  cachePolicy: "unbuffered", testFileBytes: 1_073_741_824,
  warmupSeconds: 5, measurementSeconds: 15,
  measuredAtUtc: "2026-10-17T10:00:00Z",
  measurements: [
    { profileId: "seq1m-q8t1", direction: "read", throughputMBps: 104.8576,
      iops: 100, latencyAvgMs: 0.4, latencyP95Ms: 0.72, latencyP99Ms: 1.12 }
  ]
};
test("trusted measurement maps to formatted values without rewriting units", () => {
  const value = displayedMeasurement(report, "trusted-volume-test", "seq1m-q8t1", "read");
  assert.ok(value);
  assert.match(value.mbps, /104,9/);
  assert.equal(value.iops, "100");
  assert.equal(value.avgMs, "0,4");
  assert.equal(value.p95Ms, "0,72");
});
test("sample disks, incorrect target and missing direction never show scores", () => {
  assert.equal(displayedMeasurement(report, "sample-samsung", "seq1m-q8t1", "read"), null);
  assert.equal(displayedMeasurement(report, "other", "seq1m-q8t1", "read"), null);
  assert.equal(displayedMeasurement(report, "trusted-volume-test", "seq1m-q8t1", "write"), null);
  assert.equal(displayedMeasurement(null, "trusted-volume-test", "seq1m-q8t1", "read"), null);
});
test("invalid numeric results are ignored", () => {
  const forged = { ...report, measurements: [{ ...report.measurements[0], iops: Number.NaN }] };
  assert.equal(displayedMeasurement(forged, "trusted-volume-test", "seq1m-q8t1", "read"), null);
});
