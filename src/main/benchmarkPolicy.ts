import path from "node:path";
import {
  BENCHMARK_MAX_MEASUREMENT_SECONDS, BENCHMARK_MAX_REPETITIONS,
  BENCHMARK_MAX_WARMUP_SECONDS, BENCHMARK_MIN_MEASUREMENT_SECONDS,
  BENCHMARK_PROFILES, BENCHMARK_TERMINATION_GRACE_SECONDS,
  BENCHMARK_WRITE_RATE_BYTES_PER_MS, estimateWriteImpact,
  type BenchmarkDirection, type BenchmarkProfileId
} from "../shared/benchmark";

/**
 * P2.5B.1: internal-only workload plan; never treat renderer input as a
 * verified target or grant storage access here.
 */
export type BenchmarkWorkload = Readonly<{
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
  warmupSeconds: number;
  measurementSeconds: number;
  repetitions: number;
}>;

export type PlannedDiskSpdWorkload = Readonly<{
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
  args: readonly string[];
  measurementSeconds: number;
  expectedWrite: boolean;
  maxWallClockSeconds: number;
  estimatedMaxWriteBytes: number;
}>;

const VALID_PROFILES = new Set<string>(BENCHMARK_PROFILES.map(p => p.id));

export function validateWorkload(value: unknown): BenchmarkWorkload {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Invalid benchmark workload");
  }
  const input = value as Record<string, unknown>;
  if (Object.keys(input).some(key =>
      !["profileId", "direction", "warmupSeconds", "measurementSeconds", "repetitions"].includes(key))) {
    throw new Error("Unknown benchmark workload field");
  }
  if (typeof input.profileId !== "string" || !VALID_PROFILES.has(input.profileId) ||
      (input.direction !== "read" && input.direction !== "write") ||
      !Number.isSafeInteger(input.warmupSeconds) ||
      !Number.isSafeInteger(input.measurementSeconds) ||
      !Number.isSafeInteger(input.repetitions) ||
      (input.warmupSeconds as number) < 0 || (input.warmupSeconds as number) > BENCHMARK_MAX_WARMUP_SECONDS ||
      (input.measurementSeconds as number) < BENCHMARK_MIN_MEASUREMENT_SECONDS || (input.measurementSeconds as number) > BENCHMARK_MAX_MEASUREMENT_SECONDS ||
      (input.repetitions as number) < 1 || (input.repetitions as number) > BENCHMARK_MAX_REPETITIONS) {
    throw new Error("Unsupported benchmark workload");
  }
  return Object.freeze({
    profileId: input.profileId as BenchmarkProfileId,
    direction: input.direction as BenchmarkDirection,
    warmupSeconds: input.warmupSeconds as number,
    measurementSeconds: input.measurementSeconds as number,
    repetitions: input.repetitions as number
  });
}

/**
 * No command shell, arbitrary switches, drive letters, UNC or NT device paths.
 * Only pass an independently created and verified application-owned file.
 *
 * Duration limits bound TIME, not SSD wear or total bytes written.
 */
export function buildDiskSpdFileArgs(
  trustedFilePath: string, workload: BenchmarkWorkload
): PlannedDiskSpdWorkload {
  const plan = validateWorkload(workload);
  if (typeof trustedFilePath !== "string" || !/^[A-Za-z]:[\\/]/.test(trustedFilePath) || !path.win32.isAbsolute(trustedFilePath) ||
      trustedFilePath.includes("\0") || trustedFilePath.includes("\n") ||
      trustedFilePath.includes("\r") ||
      /^\\\\[?.\\]/.test(trustedFilePath) || trustedFilePath.startsWith("\\\\") ||
      trustedFilePath.startsWith("\\Device\\") ||
      trustedFilePath.startsWith("\\??\\") ||
      path.win32.parse(trustedFilePath).root === trustedFilePath ||
      /(^|[\\/])\.\.([\\/]|$)/.test(trustedFilePath) ||
      /[:*?"<>|]/.test(trustedFilePath.slice(2))) {
    throw new Error("Benchmark target must be a local, absolute, app-owned file");
  }
  const profile = BENCHMARK_PROFILES.find(p => p.id === plan.profileId);
  if (!profile) throw new Error("Unknown DiskSpd profile");
  const size = profile.blockBytes === 1_048_576 ? "1M" : "4K";
  const args = [
    "-b" + size,
    "-o" + profile.queueDepth,
    "-t1",
    "-w" + (plan.direction === "write" ? "100" : "0"),
    "-W" + plan.warmupSeconds,
    "-d" + plan.measurementSeconds
  ];
  if (plan.direction === "write") args.push("-g" + BENCHMARK_WRITE_RATE_BYTES_PER_MS);
  args.push("-L", "-Rxml");
  if (profile.pattern === "random") args.push("-r4K");
  args.push(trustedFilePath);
  const impact = estimateWriteImpact(plan);
  const maxWallClockSeconds = impact.maxDurationSeconds;
  return Object.freeze({
    profileId: plan.profileId,
    direction: plan.direction,
    args: Object.freeze(args),
    measurementSeconds: plan.measurementSeconds,
    expectedWrite: plan.direction === "write",
    maxWallClockSeconds,
    // Include the full parent hard-deadline allowance for every repetition.
    // Consent UI can round this conservative upper estimate for display.
    estimatedMaxWriteBytes: plan.direction === "write"
      ? impact.maxWriteBytes
      : 0
  });
}
