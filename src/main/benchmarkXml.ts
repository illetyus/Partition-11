import { XMLParser, XMLValidator } from "fast-xml-parser";
import {
  BENCHMARK_ENGINE_VERSION, BENCHMARK_MAX_MEASUREMENT_SECONDS,
  BENCHMARK_MIN_MEASUREMENT_SECONDS, BENCHMARK_PROFILES,
  isFiniteMeasurement,
  type BenchmarkDirection,
  type BenchmarkMeasurement,
  type BenchmarkProfileId
} from "../shared/benchmark";

/**
 * Strict, read-only adapter for a single validated DiskSpd -Rxml run.
 * A future native worker must independently authorize and identify the
 * application-owned file BEFORE calling this function. It must NEVER
 * execute DiskSpd based on XML content.
 */
const MAX_XML_BYTES = 1024 * 1024;
const MAX_TAGS = 16_000;
const MAX_DURATION_SECONDS = 3_600;

export type TrustedDiskSpdRun = Readonly<{
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
  ownedTargetPath: string;
  pinnedEngineVersion: typeof BENCHMARK_ENGINE_VERSION;
  measurementSeconds: number;
}>;

export type ParsedDiskSpdRun = Readonly<{
  durationSeconds: number;
  engineVersion: string;
  measurement: BenchmarkMeasurement;
}>;

type XmlRecord = Record<string, unknown>;

function record(value: unknown, field: string): XmlRecord {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Invalid DiskSpd XML structure: " + field);
  }
  return value as XmlRecord;
}

function requiredString(value: unknown, field: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error("Missing DiskSpd XML field: " + field);
  }
  return value.trim();
}

function numeric(value: unknown, field: string, positive: boolean, integer: boolean): number {
  const raw = requiredString(value, field);
  if (!/^(?:0|[1-9]\d*)(?:\.\d+)?$/.test(raw)) {
    throw new Error("Invalid numeric DiskSpd XML field: " + field);
  }
  const result = Number(raw);
  if (!Number.isFinite(result) || !Number.isSafeInteger(result) && integer ||
      (integer && !Number.isInteger(result)) ||
      (positive && result <= 0) ||
      (!positive && result < 0)) {
    throw new Error("Out-of-range DiskSpd XML field: " + field);
  }
  return result;
}

function only<T>(value: T | readonly T[] | undefined, field: string): T {
  if (value === undefined || Array.isArray(value)) {
    throw new Error("Expected exactly one DiskSpd XML entry: " + field);
  }
  return value as T;
}

function percentiles(latency: XmlRecord, direction: BenchmarkDirection): { p95: number | null; p99: number | null } {
  const raw = latency.Bucket;
  if (raw === undefined) return { p95: null, p99: null };
  const buckets = Array.isArray(raw) ? raw : [raw];
  if (buckets.length > 160) throw new Error("Excessive DiskSpd histogram buckets");
  const field = direction === "read" ? "ReadMilliseconds" : "WriteMilliseconds";
  const values = new Map<number, number>();
  for (const item of buckets) {
    const bucket = record(item, "Latency.Bucket");
    const percentile = numeric(bucket.Percentile, "Latency.Bucket.Percentile", false, false);
    if (percentile > 100 || values.has(percentile)) {
      throw new Error("Repeated/out-of-range DiskSpd latency percentile");
    }
    if (bucket[field] !== undefined) {
      values.set(percentile, numeric(bucket[field], "Latency.Bucket." + field, false, false));
    }
  }
  return { p95: values.get(95) ?? null, p99: values.get(99) ?? null };
}

export function parseDiskSpdXml(xml: string, trusted: TrustedDiskSpdRun): ParsedDiskSpdRun {
  if (trusted.pinnedEngineVersion !== BENCHMARK_ENGINE_VERSION ||
      !Number.isSafeInteger(trusted.measurementSeconds) ||
      trusted.measurementSeconds < BENCHMARK_MIN_MEASUREMENT_SECONDS ||
      trusted.measurementSeconds > BENCHMARK_MAX_MEASUREMENT_SECONDS) {
    throw new Error("Invalid trusted DiskSpd run policy");
  }
  if (typeof xml !== "string" ||
      Buffer.byteLength(xml, "utf8") === 0 ||
      Buffer.byteLength(xml, "utf8") > MAX_XML_BYTES ||
      (xml.match(/</g)?.length ?? 0) > MAX_TAGS) {
    throw new Error("DiskSpd XML length/complexity limit exceeded");
  }
  // No DTDs, entities, CDATA, comments or processing instructions. XML
  // declaration alone is acceptable. Fail closed rather than expanding
  // custom entities or accepting attacker-supplied document directives.
  const payload = xml.replace(/^\uFEFF?\s*<\?xml\s+[^?]*\?>\s*/i, "").trim();
  if (/<!|<\?|&(?:#|[a-zA-Z])/.test(payload)) {
    throw new Error("DiskSpd XML declarations/entities are not supported");
  }
  if (!/^<Results(?:\s|>)/.test(payload)) {
    throw new Error("Unexpected DiskSpd XML root");
  }

  const validation = XMLValidator.validate(payload);
  if (validation !== true) throw new Error("Malformed DiskSpd XML");

  const parser = new XMLParser({
    ignoreAttributes: true,
    parseTagValue: false,
    trimValues: true,
    processEntities: false
  });
  const doc = record(parser.parse(payload), "document");
  if (Object.keys(doc).length !== 1) throw new Error("Unexpected XML top-level content");
  const root = record(doc.Results, "Results");
  const system = record(root.System, "System");
  const version = requiredString(record(system.Tool, "System.Tool").Version, "System.Tool.Version");
  if (!/^[0-9][a-zA-Z0-9.+_-]{0,31}$/.test(version) ||
      version !== trusted.pinnedEngineVersion || version !== BENCHMARK_ENGINE_VERSION) {
    throw new Error("DiskSpd version mismatch");
  }

  const spans = record(root.Profile, "Profile").TimeSpans;
  const profileSpan = record(only(record(spans, "Profile.TimeSpans").TimeSpan, "Profile.TimeSpan"), "Profile.TimeSpan");
  const profileTarget = record(
    only(record(profileSpan.Targets, "Profile.TimeSpan.Targets").Target, "Profile.TimeSpan.Target"),
    "Profile.TimeSpan.Target"
  );
  const profile = BENCHMARK_PROFILES.find(item => item.id === trusted.profileId);
  if (!profile) throw new Error("Unknown benchmark profile");
  const blockSize = numeric(profileTarget.BlockSize, "Profile.Target.BlockSize", true, true);
  const requestCount = numeric(profileTarget.RequestCount, "Profile.Target.RequestCount", true, true);
  const writeRatio = numeric(profileTarget.WriteRatio, "Profile.Target.WriteRatio", false, true);
  const random = numeric(profileTarget.Random ?? "0", "Profile.Target.Random", false, true);
  if (blockSize !== profile.blockBytes || requestCount !== profile.queueDepth ||
      writeRatio !== (trusted.direction === "write" ? 100 : 0) ||
      (profile.pattern === "random" ? random !== profile.blockBytes : random !== 0)) {
    throw new Error("DiskSpd XML workload differs from authorized profile");
  }

  const span = record(only(root.TimeSpan, "Results.TimeSpan"), "Results.TimeSpan");
  if (span.Error !== undefined) throw new Error("DiskSpd test reported an error");
  const durationSeconds = numeric(span.TestTimeSeconds, "TestTimeSeconds", true, false);
  const durationToleranceSeconds = Math.max(0.25, trusted.measurementSeconds * 0.1);
  if (durationSeconds > MAX_DURATION_SECONDS ||
      Math.abs(durationSeconds - trusted.measurementSeconds) > durationToleranceSeconds) {
    throw new Error("Unexpected benchmark duration");
  }
  // Results.TimeSpan.RequestCount is the timespan-wide shared-thread
  // setting (often 0 for per-target -o), NOT the target queue depth.
  // The authorized Q depth is checked from Profile.Target.RequestCount.
  if (numeric(span.ThreadCount, "ThreadCount", true, true) !== 1 ||
      numeric(span.RequestCount, "TimeSpan.RequestCount", false, true) > profile.queueDepth) {
    throw new Error("Unexpected DiskSpd concurrency");
  }
  const thread = record(only(span.Thread, "TimeSpan.Thread"), "TimeSpan.Thread");
  const target = record(only(thread.Target, "Thread.Target"), "Thread.Target");
  if (requiredString(target.Path, "Thread.Target.Path") !== trusted.ownedTargetPath) {
    throw new Error("DiskSpd result target is not the authorized scratch file");
  }
  const readBytes = numeric(target.ReadBytes, "ReadBytes", false, true);
  const writeBytes = numeric(target.WriteBytes, "WriteBytes", false, true);
  const readCount = numeric(target.ReadCount, "ReadCount", false, true);
  const writeCount = numeric(target.WriteCount, "WriteCount", false, true);
  const bytes = trusted.direction === "read" ? readBytes : writeBytes;
  const count = trusted.direction === "read" ? readCount : writeCount;
  if (!bytes || !count ||
      (trusted.direction === "read" ? writeBytes !== 0 || writeCount !== 0 : readBytes !== 0 || readCount !== 0) ||
      numeric(target.BytesCount, "BytesCount", false, true) !== readBytes + writeBytes ||
      numeric(target.IOCount, "IOCount", false, true) !== readCount + writeCount) {
    throw new Error("Mixed, empty or inconsistent DiskSpd I/O counters");
  }
  const latency = record(span.Latency, "TimeSpan.Latency");
  const avgField = trusted.direction === "read" ? "AverageReadMilliseconds" : "AverageWriteMilliseconds";
  const avg = numeric(latency[avgField], "Latency." + avgField, false, false);
  const { p95, p99 } = percentiles(latency, trusted.direction);
  if (p95 !== null && p99 !== null && p99 < p95) {
    throw new Error("Invalid latency percentile ordering");
  }
  const measurement: BenchmarkMeasurement = {
    profileId: trusted.profileId,
    direction: trusted.direction,
    throughputMBps: bytes / durationSeconds / 1_000_000,
    iops: count / durationSeconds,
    latencyAvgMs: avg,
    latencyP95Ms: p95,
    latencyP99Ms: p99
  };
  if (!isFiniteMeasurement(measurement)) throw new Error("Invalid DiskSpd measurement");
  return { durationSeconds, engineVersion: version, measurement };
}
