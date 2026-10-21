import { createHmac, randomBytes } from "node:crypto";
import {
  BENCHMARK_ENGINE_VERSION, BENCHMARK_PROFILES, estimateWriteImpact,
  type BenchmarkDirection, type BenchmarkProfileId, type BenchmarkReport,
  type BenchmarkRunResult, type BenchmarkStartRequest, type WorkerBenchmarkReport
} from "../shared/benchmark";

const SCRATCH_FILE_BYTES = 1024 * 1024 * 1024;
const WARMUP_SECONDS = 5;
const MEASUREMENT_SECONDS = 15;
const REPETITIONS = 1;
const MAX_REMEMBERED_REQUEST_IDS = 128;
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export type ConsumedBenchmarkSelection = Readonly<{
  selection: Readonly<{
    volumeOrdinal: number;
    mountPoint: string;
    fileSystem: string;
    totalBytes: string;
    availableBytes: string;
    diskOrdinal: number;
    partitionNumber: number;
    identityHash: string;
  }>;
  identityKey: Buffer;
}>;

export type BenchmarkAuthorizationEnvelope = Readonly<{
  protocolVersion: 1;
  operation: "runFileBenchmark";
  requestId: string;
  volumeOrdinal: number;
  diskOrdinal: number;
  partitionNumber: number;
  identityKey: string;
  identityHash: string;
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
  authorizationKey: string;
  authorizationTag: string;
}>;

export type BenchmarkConsentDetails = Readonly<{
  mountPoint: string;
  fileSystem: string;
  profileId: BenchmarkProfileId;
  direction: BenchmarkDirection;
  scratchFileBytes: number;
  warmupSeconds: number;
  measurementSeconds: number;
  maxDurationSeconds: number;
  maxWriteBytes: number;
}>;

export type BenchmarkWorkerResult =
  | Readonly<{ status: "completed"; report: WorkerBenchmarkReport }>
  | Readonly<{ status: "cancelled" }>
  | Readonly<{ status: "error"; error: "worker_failure" | "invalid_response" | "cleanup_required" }>;

export type BenchmarkRunDependencies = Readonly<{
  /** The production gate stays false until every P2.5 CI and lifecycle gate passes. */
  runtimeAvailable: () => boolean;
  consumeSelection: (token: string) => ConsumedBenchmarkSelection | null;
  confirm: (details: BenchmarkConsentDetails) => Promise<boolean>;
  execute: (request: BenchmarkAuthorizationEnvelope, signal: AbortSignal) => Promise<BenchmarkWorkerResult>;
  createAuthorizationKey?: () => Buffer;
}>;

const PROFILE_IDS = new Set<string>(BENCHMARK_PROFILES.map(profile => profile.id));
const PROFILE_TAG: Readonly<Record<BenchmarkProfileId, number>> = Object.freeze({
  "seq1m-q8t1": 1,
  "seq1m-q1t1": 2,
  "rnd4k-q32t1": 3,
  "rnd4k-q1t1": 4
});

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  return keys.length === sortedExpected.length && keys.every((key, index) => key === sortedExpected[index]);
}

const REPORT_KEYS = [
  "protocolVersion", "source", "engine", "engineVersion", "filesystem", "targetVolumeId",
  "cachePolicy", "testFileBytes", "warmupSeconds", "measurementSeconds", "measuredAtUtc", "measurements"
] as const;
const MEASUREMENT_KEYS = [
  "profileId", "direction", "throughputMBps", "iops", "latencyAvgMs", "latencyP95Ms", "latencyP99Ms"
] as const;

function validUtcTimestamp(value: unknown): value is string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(value)) return false;
  const date = new Date(value);
  return Number.isFinite(date.getTime()) && date.toISOString() === value;
}

function validPositiveFinite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function validOptionalLatency(value: unknown): value is number | null {
  return value === null || (typeof value === "number" && Number.isFinite(value) && value >= 0);
}

/** Validate worker-controlled fields in main, then project a renderer-safe report. */
function sanitizeWorkerReport(
  value: unknown,
  request: BenchmarkStartRequest,
  selection: ConsumedBenchmarkSelection
): BenchmarkReport | null {
  if (!isRecord(value) || !hasExactKeys(value, REPORT_KEYS) ||
      value.protocolVersion !== 1 || value.source !== "measured" || value.engine !== "DiskSpd" ||
      value.engineVersion !== BENCHMARK_ENGINE_VERSION ||
      value.filesystem !== selection.selection.fileSystem ||
      value.targetVolumeId !== selection.selection.identityHash ||
      value.cachePolicy !== (request.direction === "read" ? "unbuffered" : "write-through") ||
      value.testFileBytes !== SCRATCH_FILE_BYTES || value.warmupSeconds !== WARMUP_SECONDS ||
      value.measurementSeconds !== MEASUREMENT_SECONDS || !validUtcTimestamp(value.measuredAtUtc) ||
      !Array.isArray(value.measurements) || value.measurements.length !== 1) {
    return null;
  }
  const measurement = value.measurements[0];
  if (!isRecord(measurement) || !hasExactKeys(measurement, MEASUREMENT_KEYS) ||
      measurement.profileId !== request.profileId || measurement.direction !== request.direction ||
      !validPositiveFinite(measurement.throughputMBps) || !validPositiveFinite(measurement.iops) ||
      !validPositiveFinite(measurement.latencyAvgMs) ||
      !validOptionalLatency(measurement.latencyP95Ms) || !validOptionalLatency(measurement.latencyP99Ms) ||
      (measurement.latencyP95Ms !== null && measurement.latencyP99Ms !== null &&
        measurement.latencyP99Ms < measurement.latencyP95Ms)) {
    return null;
  }

  // The keyed volume identity is checked above and must never cross into renderer IPC.
  // The one-use selection token is already renderer-visible and safely binds this result.
  return Object.freeze({
    protocolVersion: 1,
    source: "measured",
    engine: "DiskSpd",
    engineVersion: BENCHMARK_ENGINE_VERSION,
    filesystem: selection.selection.fileSystem,
    targetVolumeId: request.selectionToken,
    cachePolicy: request.direction === "read" ? "unbuffered" : "write-through",
    testFileBytes: SCRATCH_FILE_BYTES,
    warmupSeconds: WARMUP_SECONDS,
    measurementSeconds: MEASUREMENT_SECONDS,
    measuredAtUtc: value.measuredAtUtc,
    measurements: Object.freeze([Object.freeze({
      profileId: request.profileId,
      direction: request.direction,
      throughputMBps: measurement.throughputMBps,
      iops: measurement.iops,
      latencyAvgMs: measurement.latencyAvgMs,
      latencyP95Ms: measurement.latencyP95Ms,
      latencyP99Ms: measurement.latencyP99Ms
    })])
  });
}

function validateWorkerResult(
  value: unknown,
  request: BenchmarkStartRequest,
  selection: ConsumedBenchmarkSelection
): BenchmarkRunResult | null {
  if (!isRecord(value) || typeof value.status !== "string") return null;
  if (value.status === "completed") {
    if (!hasExactKeys(value, ["status", "report"])) return null;
    const report = sanitizeWorkerReport(value.report, request, selection);
    return report ? { status: "completed", report } : null;
  }
  if (value.status === "cancelled") {
    return hasExactKeys(value, ["status"]) ? { status: "cancelled" } : null;
  }
  if (value.status === "error" && hasExactKeys(value, ["status", "error"]) &&
      (value.error === "worker_failure" || value.error === "invalid_response" || value.error === "cleanup_required")) {
    return { status: "error", error: value.error };
  }
  return null;
}

/** Renderer-facing requests deliberately contain no paths, switches or workload bounds. */
export function validateBenchmarkStartRequest(value: unknown): BenchmarkStartRequest {
  if (!isRecord(value) || !hasExactKeys(value, ["requestId", "selectionToken", "profileId", "direction"]) ||
      typeof value.requestId !== "string" || !UUID_V4.test(value.requestId) ||
      typeof value.selectionToken !== "string" || !UUID_V4.test(value.selectionToken) ||
      typeof value.profileId !== "string" || !PROFILE_IDS.has(value.profileId) ||
      (value.direction !== "read" && value.direction !== "write")) {
    throw new Error("Invalid benchmark start request");
  }
  return Object.freeze({
    requestId: value.requestId,
    selectionToken: value.selectionToken,
    profileId: value.profileId as BenchmarkProfileId,
    direction: value.direction
  });
}

function authorizationMessage(request: BenchmarkStartRequest, selection: ConsumedBenchmarkSelection): Buffer {
  const selected = selection.selection;
  const requestId = Buffer.from(request.requestId, "ascii");
  const requestIdLength = Buffer.alloc(2);
  requestIdLength.writeUInt16LE(requestId.length);
  const numericFields = Buffer.alloc(12);
  numericFields.writeUInt32LE(selected.volumeOrdinal, 0);
  numericFields.writeUInt32LE(selected.diskOrdinal, 4);
  numericFields.writeUInt32LE(selected.partitionNumber, 8);
  const identityKey = Buffer.from(selection.identityKey);
  const identityHash = Buffer.from(selected.identityHash, "hex");
  const workload = Buffer.from([
    PROFILE_TAG[request.profileId],
    request.direction === "read" ? 1 : 2
  ]);
  const message = Buffer.concat([
    Buffer.from("Partition11 benchmark authorization v1\0", "utf8"),
    requestIdLength,
    requestId,
    numericFields,
    identityKey,
    identityHash,
    workload
  ]);
  identityKey.fill(0);
  identityHash.fill(0);
  return message;
}

/** Builds the exact version-1 envelope accepted by the Windows agent. */
export function buildBenchmarkAuthorizationEnvelope(
  request: BenchmarkStartRequest,
  selection: ConsumedBenchmarkSelection,
  authorizationKey: Uint8Array = randomBytes(32)
): BenchmarkAuthorizationEnvelope {
  const selected = selection.selection;
  if (authorizationKey.length !== 32 || selection.identityKey.length !== 32 ||
      !Number.isInteger(selected.volumeOrdinal) || selected.volumeOrdinal < 1 || selected.volumeOrdinal > 256 ||
      !Number.isInteger(selected.diskOrdinal) || selected.diskOrdinal < 1 || selected.diskOrdinal > 256 ||
      !Number.isInteger(selected.partitionNumber) || selected.partitionNumber < 1 || selected.partitionNumber > 128 ||
      !/^[0-9a-f]{64}$/.test(selected.identityHash)) {
    throw new Error("Invalid benchmark authorization context");
  }
  const key = Buffer.from(authorizationKey);
  const authorizationKeyHex = key.toString("hex");
  const payload = authorizationMessage(request, selection);
  let authorizationTag: string;
  try {
    authorizationTag = createHmac("sha256", key).update(payload).digest("hex");
  } finally {
    key.fill(0);
    payload.fill(0);
  }
  return Object.freeze({
    protocolVersion: 1,
    operation: "runFileBenchmark",
    requestId: request.requestId,
    volumeOrdinal: selected.volumeOrdinal,
    diskOrdinal: selected.diskOrdinal,
    partitionNumber: selected.partitionNumber,
    identityKey: selection.identityKey.toString("hex"),
    identityHash: selected.identityHash,
    profileId: request.profileId,
    direction: request.direction,
    authorizationKey: authorizationKeyHex,
    authorizationTag
  });
}

type ActiveRun = Readonly<{ requestId: string; abort: AbortController }>;

/** Main-process-only single-run, one-use-selection consent controller. */
export class BenchmarkRunController {
  private active: ActiveRun | undefined;
  private cleanupBlocked = false;
  private readonly seenRequestIds = new Set<string>();
  private readonly createAuthorizationKey: () => Buffer;

  constructor(private readonly dependencies: BenchmarkRunDependencies) {
    this.createAuthorizationKey = dependencies.createAuthorizationKey ?? (() => randomBytes(32));
  }

  async start(value: unknown): Promise<BenchmarkRunResult> {
    let request: BenchmarkStartRequest;
    try {
      request = validateBenchmarkStartRequest(value);
    } catch {
      return { status: "invalid_request" };
    }
    if (this.active) return { status: "busy" };
    if (this.cleanupBlocked) return { status: "error", error: "cleanup_required" };
    if (this.seenRequestIds.has(request.requestId)) return { status: "stale_request" };
    let workerStarted = false;
    try {
      if (!this.dependencies.runtimeAvailable()) return { status: "unavailable" };
    } catch {
      return { status: "unavailable" };
    }

    this.rememberRequestId(request.requestId);
    let selection: ConsumedBenchmarkSelection | null;
    try {
      selection = this.dependencies.consumeSelection(request.selectionToken);
    } catch {
      return { status: "stale_selection" };
    }
    if (!selection) return { status: "stale_selection" };
    if (!isValidConsumedSelection(selection)) {
      selection.identityKey.fill(0);
      return { status: "stale_selection" };
    }

    const abort = new AbortController();
    const active = { requestId: request.requestId, abort };
    this.active = active;
    const impact = estimateWriteImpact({
      warmupSeconds: WARMUP_SECONDS,
      measurementSeconds: MEASUREMENT_SECONDS,
      repetitions: REPETITIONS
    });
    const consentDetails: BenchmarkConsentDetails = Object.freeze({
      mountPoint: selection.selection.mountPoint,
      fileSystem: selection.selection.fileSystem,
      profileId: request.profileId,
      direction: request.direction,
      scratchFileBytes: SCRATCH_FILE_BYTES,
      warmupSeconds: WARMUP_SECONDS,
      measurementSeconds: MEASUREMENT_SECONDS,
      maxDurationSeconds: impact.maxDurationSeconds,
      maxWriteBytes: request.direction === "write" ? impact.maxWriteBytes : 0
    });

    try {
      const approved = await this.dependencies.confirm(consentDetails);
      if (abort.signal.aborted) return { status: "cancelled" };
      if (!approved) return { status: "consent_declined" };
      const authorizationKey = this.createAuthorizationKey();
      let envelope: BenchmarkAuthorizationEnvelope;
      try {
        envelope = buildBenchmarkAuthorizationEnvelope(request, selection, authorizationKey);
      } finally {
        authorizationKey.fill(0);
      }
      workerStarted = true;
      const rawResult: unknown = await this.dependencies.execute(envelope, abort.signal);
      const result = validateWorkerResult(rawResult, request, selection);
      if (!result) {
        this.cleanupBlocked = true;
        return { status: "error", error: abort.signal.aborted ? "cleanup_required" : "invalid_response" };
      }
      if (result.status === "error" && (result.error === "cleanup_required" || result.error === "invalid_response")) {
        this.cleanupBlocked = true;
      }
      if (abort.signal.aborted) {
        if (result.status === "cancelled") {
          return result;
        }
        this.cleanupBlocked = true;
        return { status: "error", error: "cleanup_required" };
      }
      if (result.status === "cancelled") {
        this.cleanupBlocked = true;
        return { status: "error", error: "invalid_response" };
      }
      return result;
    } catch {
      if (workerStarted) this.cleanupBlocked = true;
      return { status: "error", error: abort.signal.aborted || workerStarted ? "cleanup_required" : "worker_failure" };
    } finally {
      selection.identityKey.fill(0);
      if (this.active === active) this.active = undefined;
    }
  }

  cancel(requestId: string): BenchmarkRunResult {
    if (!this.active || requestId !== this.active.requestId) return { status: "stale_cancel" };
    this.active.abort.abort();
    return { status: "cancelling" };
  }

  cancelActive(): void {
    this.active?.abort.abort();
  }

  private rememberRequestId(requestId: string): void {
    this.seenRequestIds.add(requestId);
    if (this.seenRequestIds.size > MAX_REMEMBERED_REQUEST_IDS) {
      const oldest = this.seenRequestIds.values().next().value as string | undefined;
      if (oldest) this.seenRequestIds.delete(oldest);
    }
  }
}

function isValidConsumedSelection(selection: ConsumedBenchmarkSelection): boolean {
  const selected = selection.selection;
  if (!Buffer.isBuffer(selection.identityKey) || selection.identityKey.length !== 32 ||
      !Number.isInteger(selected.volumeOrdinal) || selected.volumeOrdinal < 1 || selected.volumeOrdinal > 256 ||
      !Number.isInteger(selected.diskOrdinal) || selected.diskOrdinal < 1 || selected.diskOrdinal > 256 ||
      !Number.isInteger(selected.partitionNumber) || selected.partitionNumber < 1 || selected.partitionNumber > 128 ||
      typeof selected.mountPoint !== "string" || !/^[A-Za-z]:\\$/.test(selected.mountPoint) ||
      typeof selected.fileSystem !== "string" || selected.fileSystem.toUpperCase() !== "NTFS") {
    return false;
  }
  if (!/^[0-9a-f]{64}$/.test(selected.identityHash) ||
      !/^(0|[1-9][0-9]{0,19})$/.test(selected.totalBytes) ||
      !/^(0|[1-9][0-9]{0,19})$/.test(selected.availableBytes)) return false;
  try {
    const totalBytes = BigInt(selected.totalBytes);
    const availableBytes = BigInt(selected.availableBytes);
    return totalBytes > 0n && availableBytes <= totalBytes && availableBytes >= 2n * 1024n * 1024n * 1024n;
  } catch {
    return false;
  }
}
