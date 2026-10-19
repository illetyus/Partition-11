import { spawn, type ChildProcess, type SpawnOptions } from "node:child_process";
import { existsSync } from "node:fs";
import { randomUUID } from "node:crypto";
import type {
  InventoryAvailability,
  ReadOnlyDisk,
  ReadOnlyDiskInventory
} from "../shared/protocol";

const AGENT_TIMEOUT_MS = 35_000;
const MAX_RESPONSE_BYTES = 256 * 1024;
const MAX_DISKS = 256;
const BUS_TYPES = new Set([
  "scsi", "atapi", "ata", "ieee1394", "ssa", "fibre", "usb", "raid",
  "iscsi", "sas", "sata", "sd", "mmc", "virtual", "nvme", "scm", "ufs"
]);
const DISK_AVAILABILITY = new Set<InventoryAvailability>([
  "ready", "access_denied", "device_unavailable", "unsupported",
  "identity_changed", "scan_timeout", "agent_failure"
]);
const AGENT_ERRORS = new Set(["access_denied", "agent_failure", "scan_timeout"]);

type AgentSuccess = Readonly<{ ok: true; disks: readonly ReadOnlyDisk[] }>;
type AgentFailure = Readonly<{ ok: false; error: "access_denied" | "agent_failure" | "scan_timeout" }>;
type AgentResponse = AgentSuccess | AgentFailure;

type SpawnFactory = (executable: string, args: string[], options: SpawnOptions) => ChildProcess;
type AgentRunResult =
  | { kind: "response"; value: unknown }
  | { kind: "timeout" }
  | { kind: "spawn-error" }
  | { kind: "exit-error" }
  | { kind: "oversize" };
type ServiceOptions = Readonly<{
  exists?: (path: string) => boolean;
  spawn?: SpawnFactory;
  timeoutMs?: number;
  now?: () => Date;
}>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function validSectorSize(value: unknown): value is number | null {
  return value === null || (
    typeof value === "number" && Number.isInteger(value) &&
    value >= 512 && value <= 65_536 && (value & (value - 1)) === 0
  );
}

function validCapacity(value: unknown): value is string | null {
  if (value === null) return true;
  if (typeof value !== "string" || !/^[1-9][0-9]{0,19}$/.test(value)) return false;
  try { return BigInt(value) <= 18_446_744_073_709_551_615n; } catch { return false; }
}

function validateDisk(value: unknown, expectedOrdinal: number): ReadOnlyDisk | null {
  if (!isRecord(value) || !hasExactKeys(value, [
    "ordinal", "availability", "capacityBytes", "logicalSectorSize",
    "physicalSectorSize", "busType", "partitionStyle", "partitionCount"
  ])) return null;
  if (value.ordinal !== expectedOrdinal ||
      typeof value.availability !== "string" || !DISK_AVAILABILITY.has(value.availability as InventoryAvailability) ||
      !validCapacity(value.capacityBytes) ||
      !validSectorSize(value.logicalSectorSize) || !validSectorSize(value.physicalSectorSize) ||
      !(value.busType === null || (typeof value.busType === "string" && BUS_TYPES.has(value.busType))) ||
      !(value.partitionStyle === null || value.partitionStyle === "gpt" || value.partitionStyle === "mbr" || value.partitionStyle === "raw") ||
      !(value.partitionCount === null || (typeof value.partitionCount === "number" && Number.isInteger(value.partitionCount) && value.partitionCount >= 0 && value.partitionCount <= 128))) {
    return null;
  }
  if (value.availability !== "ready" && (
    value.capacityBytes !== null || value.logicalSectorSize !== null ||
    value.physicalSectorSize !== null || value.busType !== null ||
    value.partitionStyle !== null || value.partitionCount !== null
  )) return null;
  return value as unknown as ReadOnlyDisk;
}

/** Validate the complete agent envelope; unknown fields and stale IDs fail closed. */
export function validateAgentResponse(value: unknown, requestId: string): AgentResponse | null {
  if (!isRecord(value) || value.protocolVersion !== 1 || value.requestId !== requestId || typeof value.ok !== "boolean") {
    return null;
  }
  if (value.ok === false) {
    if (!hasExactKeys(value, ["protocolVersion", "requestId", "ok", "error"]) ||
        typeof value.error !== "string" || !AGENT_ERRORS.has(value.error)) return null;
    return { ok: false, error: value.error as AgentFailure["error"] };
  }
  if (!hasExactKeys(value, ["protocolVersion", "requestId", "ok", "disks"]) ||
      !Array.isArray(value.disks) || value.disks.length > MAX_DISKS) return null;
  const disks: ReadOnlyDisk[] = [];
  for (let index = 0; index < value.disks.length; index += 1) {
    const disk = validateDisk(value.disks[index], index + 1);
    if (!disk) return null;
    disks.push(disk);
  }
  return { ok: true, disks };
}

type InventoryError = Extract<ReadOnlyDiskInventory, { status: "error" }>["error"];

function errorResult(error: InventoryError): ReadOnlyDiskInventory {
  return { protocolVersion: 1, status: "error", error };
}

function readOneJsonLine(chunks: Buffer[]): unknown | null {
  const bytes = Buffer.concat(chunks);
  let text = bytes.toString("utf8");
  if (text.endsWith("\n")) text = text.slice(0, -1);
  if (text.endsWith("\r")) text = text.slice(0, -1);
  if (text.length === 0 || text.includes("\n") || text.includes("\r")) return null;
  try { return JSON.parse(text) as unknown; } catch { return null; }
}

function runAgent(
  executable: string,
  requestId: string,
  spawnProcess: SpawnFactory,
  timeoutMs: number
): Promise<AgentRunResult> {
  return new Promise(resolve => {
    let settled = false;
    let size = 0;
    const chunks: Buffer[] = [];
    let timer: ReturnType<typeof setTimeout> | undefined;
    let child: ChildProcess;
    const finish = (result: AgentRunResult) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      resolve(result);
    };
    try {
      child = spawnProcess(executable, ["inventory"], {
        shell: false,
        windowsHide: true,
        stdio: ["pipe", "pipe", "ignore"]
      });
    } catch {
      finish({ kind: "spawn-error" });
      return;
    }
    if (!child.stdin || !child.stdout) {
      child.kill();
      finish({ kind: "spawn-error" });
      return;
    }
    timer = setTimeout(() => {
      child.kill();
      finish({ kind: "timeout" });
    }, timeoutMs);
    child.stdout.on("data", (chunk: Buffer | string) => {
      const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      size += bytes.length;
      if (size > MAX_RESPONSE_BYTES) {
        child.kill();
        finish({ kind: "oversize" });
      } else {
        chunks.push(bytes);
      }
    });
    child.once("error", () => finish({ kind: "spawn-error" }));
    child.once("close", code => {
      if (settled) return;
      if (code !== 0) {
        finish({ kind: "exit-error" });
        return;
      }
      finish({ kind: "response", value: readOneJsonLine(chunks) });
    });
    child.stdin.end(JSON.stringify({
      protocolVersion: 1,
      requestId,
      operation: "inspectPhysicalDisks"
    }) + "\n");
  });
}

export class ReadOnlyDiskInventoryService {
  private active: Promise<ReadOnlyDiskInventory> | undefined;
  private readonly exists: (path: string) => boolean;
  private readonly spawnProcess: SpawnFactory;
  private readonly timeoutMs: number;
  private readonly now: () => Date;

  constructor(readonly executablePath: string, options: ServiceOptions = {}) {
    this.exists = options.exists ?? existsSync;
    this.spawnProcess = options.spawn ?? ((file, args, spawnOptions) => spawn(file, args, spawnOptions));
    this.timeoutMs = options.timeoutMs ?? AGENT_TIMEOUT_MS;
    this.now = options.now ?? (() => new Date());
  }

  scan(): Promise<ReadOnlyDiskInventory> {
    if (this.active) return this.active;
    this.active = this.run().finally(() => { this.active = undefined; });
    return this.active;
  }

  private async run(): Promise<ReadOnlyDiskInventory> {
    if (!this.exists(this.executablePath)) return errorResult("agent_unavailable");
    const requestId = randomUUID();
    const result = await runAgent(this.executablePath, requestId, this.spawnProcess, this.timeoutMs);
    if (result.kind === "timeout") return errorResult("timeout");
    if (result.kind === "spawn-error") return errorResult("agent_unavailable");
    if (result.kind === "oversize") return errorResult("invalid_response");
    if (result.kind === "exit-error") return errorResult("agent_failure");
    const message = validateAgentResponse(result.value, requestId);
    if (!message) return errorResult("invalid_response");
    if (!message.ok) {
      if (message.error === "scan_timeout") return errorResult("timeout");
      if (message.error === "access_denied") return errorResult("access_denied");
      return errorResult("agent_failure");
    }
    return {
      protocolVersion: 1,
      status: "ok",
      scannedAt: this.now().toISOString(),
      disks: message.disks
    };
  }
}
