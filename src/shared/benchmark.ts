/**
 * P2.5A typed, inert benchmark contract.
 * This module contains NO process execution, filesystem paths, or disk access.
 * Real measurements must be supplied by a separately audited worker later.
 */
export const BENCHMARK_PROTOCOL_VERSION = 1 as const;
/** XML result version reported by the bundled, hash-pinned DiskSpd AMD64 build. */
export const BENCHMARK_ENGINE_VERSION = "2.3.0" as const;

export type BenchmarkDirection = "read" | "write";
export type BenchmarkPattern = "sequential" | "random";
export type BenchmarkProfileId = "seq1m-q8t1" | "seq1m-q1t1" | "rnd4k-q32t1" | "rnd4k-q1t1";

export type BenchmarkStartRequest = Readonly<{
  requestId: string;
  selectionToken: string;
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
}>;

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

export const BENCHMARK_MAX_WARMUP_SECONDS = 15;
export const BENCHMARK_MIN_MEASUREMENT_SECONDS = 2;
export const BENCHMARK_MAX_MEASUREMENT_SECONDS = 60;
export const BENCHMARK_MAX_REPETITIONS = 1;
export const BENCHMARK_TERMINATION_GRACE_SECONDS = 15;
export const BENCHMARK_WRITE_RATE_BYTES_PER_MS = 100_000;

export type WriteImpactInput = Readonly<{
  warmupSeconds: number;
  measurementSeconds: number;
  repetitions: number;
}>;

export type WriteImpactEstimate = Readonly<{
  maxWriteBytes: number;
  maxDurationSeconds: number;
}>;

/** Conservative disclosure bound for one approved file-backed write profile. */
export function estimateWriteImpact(input: WriteImpactInput): WriteImpactEstimate {
  if (!Number.isSafeInteger(input.warmupSeconds) || input.warmupSeconds < 0 ||
      input.warmupSeconds > BENCHMARK_MAX_WARMUP_SECONDS ||
      !Number.isSafeInteger(input.measurementSeconds) ||
      input.measurementSeconds < BENCHMARK_MIN_MEASUREMENT_SECONDS ||
      input.measurementSeconds > BENCHMARK_MAX_MEASUREMENT_SECONDS ||
      !Number.isSafeInteger(input.repetitions) || input.repetitions < 1 ||
      input.repetitions > BENCHMARK_MAX_REPETITIONS) {
    throw new Error("Unsupported benchmark impact settings");
  }
  const maxDurationSeconds = (
    input.warmupSeconds + input.measurementSeconds + BENCHMARK_TERMINATION_GRACE_SECONDS
  ) * input.repetitions;
  return Object.freeze({
    maxWriteBytes: BENCHMARK_WRITE_RATE_BYTES_PER_MS * 1_000 * maxDurationSeconds,
    maxDurationSeconds
  });
}

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
  /** Renderer-visible one-use selection token; never a stable hardware identity. */
  targetVolumeId: string;
  cachePolicy: "unbuffered" | "buffered" | "write-through";
  testFileBytes: number;
  warmupSeconds: number;
  measurementSeconds: number;
  measuredAtUtc: string;
  measurements: readonly BenchmarkMeasurement[];
}>;

declare const workerVolumeIdentityHashBrand: unique symbol;
/** Internal worker output carries a keyed identity hash that main must validate and strip. */
export type WorkerVolumeIdentityHash = string & { readonly [workerVolumeIdentityHashBrand]: true };
export type WorkerBenchmarkReport = Omit<BenchmarkReport, "targetVolumeId"> & Readonly<{
  targetVolumeId: WorkerVolumeIdentityHash;
}>;

export type BenchmarkRunResult =
  | Readonly<{ status: "unavailable" | "busy" | "invalid_request" | "stale_request" | "stale_selection" | "consent_declined" | "cancelled" | "cancelling" | "stale_cancel" }>
  | Readonly<{ status: "completed"; report: BenchmarkReport }>
  | Readonly<{ status: "error"; error: "worker_failure" | "invalid_response" | "cleanup_required" }>;

/** A displayed result must be a genuine, validated measurement. */
export function isFiniteMeasurement(value: BenchmarkMeasurement): boolean {
  const fields = [value.throughputMBps, value.iops, value.latencyAvgMs, value.latencyP95Ms, value.latencyP99Ms];
  return fields.every(item => item === null || (Number.isFinite(item) && item >= 0)) &&
    BENCHMARK_PROFILES.some(profile => profile.id === value.profileId) &&
    (value.direction === "read" || value.direction === "write");
}
