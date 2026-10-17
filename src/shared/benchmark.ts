/**
 * P2.5A typed, inert benchmark contract.
 * This module contains NO process execution, filesystem paths, or disk access.
 * Real measurements must be supplied by a separately audited worker later.
 */
export const BENCHMARK_PROTOCOL_VERSION = 1 as const;

export type BenchmarkDirection = "read" | "write";
export type BenchmarkPattern = "sequential" | "random";
export type BenchmarkProfileId = "seq1m-q8t1" | "seq1m-q1t1" | "rnd4k-q32t1" | "rnd4k-q1t1";

export type BenchmarkProfile = Readonly<{
  id: BenchmarkProfileId;
  label: string;
  pattern: BenchmarkPattern;
  blockBytes: number;
  queueDepth: number;
  threads: 1;
}>;

export const BENCHMARK_PROFILES: readonly BenchmarkProfile[] = Object.freeze([
  { id: "seq1m-q8t1", label: "SEQ1M Q8T1", pattern: "sequential", blockBytes: 1048576, queueDepth: 8, threads: 1 },
  { id: "seq1m-q1t1", label: "SEQ1M Q1T1", pattern: "sequential", blockBytes: 1048576, queueDepth: 1, threads: 1 },
  { id: "rnd4k-q32t1", label: "RND4K Q32T1", pattern: "random", blockBytes: 4096, queueDepth: 32, threads: 1 },
  { id: "rnd4k-q1t1", label: "RND4K Q1T1", pattern: "random", blockBytes: 4096, queueDepth: 1, threads: 1 }
]);

export type BenchmarkAvailability = Readonly<{
  protocolVersion: typeof BENCHMARK_PROTOCOL_VERSION;
  phase: "P2.5A";
  mode: "preview";
  engineAvailable: false;
  canRead: false;
  canWrite: false;
}>;

export const BENCHMARK_PREVIEW_AVAILABILITY: BenchmarkAvailability = Object.freeze({
  protocolVersion: BENCHMARK_PROTOCOL_VERSION,
  phase: "P2.5A",
  mode: "preview",
  engineAvailable: false,
  canRead: false,
  canWrite: false
});

export type BenchmarkMeasurement = Readonly<{
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
  throughputMBps: number;
  iops: number;
  latencyAvgMs: number;
  latencyP95Ms: number | null;
  latencyP99Ms: number | null;
}>;

export type BenchmarkReport = Readonly<{
  protocolVersion: typeof BENCHMARK_PROTOCOL_VERSION;
  source: "measured";
  engine: "DiskSpd";
  engineVersion: string;
  filesystem: string;
  targetVolumeId: string;
  cachePolicy: "unbuffered" | "buffered" | "write-through";
  testFileBytes: number;
  warmupSeconds: number;
  measurementSeconds: number;
  measuredAtUtc: string;
  measurements: readonly BenchmarkMeasurement[];
}>;

/** A displayed result must be a genuine, validated measurement. */
export function isFiniteMeasurement(value: BenchmarkMeasurement): boolean {
  const fields = [value.throughputMBps, value.iops, value.latencyAvgMs, value.latencyP95Ms, value.latencyP99Ms];
  return fields.every(item => item === null || (Number.isFinite(item) && item >= 0)) &&
    BENCHMARK_PROFILES.some(profile => profile.id === value.profileId) &&
    (value.direction === "read" || value.direction === "write");
}
