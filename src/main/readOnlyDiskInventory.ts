import { spawn, type ChildProcess, type SpawnOptions } from "node:child_process";
import { existsSync } from "node:fs";
import { randomUUID } from "node:crypto";
import type {
  InventoryAvailability,
  ReadOnlyDisk,
  ReadOnlyDiskInventory,
  ReadOnlyVolume,
  VolumeEligibilityReason
} from "../shared/protocol";

const AGENT_TIMEOUT_MS = 35_000;
const AGENT_HARD_DEADLINE_EXIT_CODE = 124;
const MAX_RESPONSE_BYTES = 256 * 1024;
const MAX_DISKS = 256;
const MAX_VOLUMES = 256;
const BUS_TYPES = new Set([
  "scsi", "atapi", "ata", "ieee1394", "ssa", "fibre", "usb", "raid",
  "iscsi", "sas", "sata", "sd", "mmc", "virtual", "nvme", "scm", "ufs"
]);
const DISK_AVAILABILITY = new Set<InventoryAvailability>([
  "ready", "access_denied", "device_unavailable", "unsupported",
  "identity_changed", "scan_timeout", "agent_failure"
]);
const AGENT_ERRORS = new Set(["access_denied", "agent_failure", "scan_timeout"]);
const DRIVE_TYPES = new Set(["unknown", "no_root", "removable", "fixed", "remote", "cdrom", "ramdisk"]);
const FILE_SYSTEMS = new Set(["NTFS", "ReFS", "exFAT", "FAT", "FAT32"]);
const VOLUME_REASONS = new Set<VolumeEligibilityReason>([
  "eligible", "system_volume", "system_volume_unknown", "no_drive_letter_mount", "ambiguous_mount",
  "unsupported_drive_type", "unsupported_filesystem", "read_only", "missing_space_metadata",
  "insufficient_space", "multiple_extents", "missing_extents", "invalid_extent", "unknown_disk",
  "ambiguous_disk", "boot_disk", "partition_unmapped", "protected_partition", "unsupported_partition"
]);

type AgentVolume = Omit<ReadOnlyVolume, "selectionToken">;
type VolumeSelection = Readonly<{
  volumeOrdinal: number;
  mountPoint: string;
  fileSystem: NonNullable<AgentVolume["fileSystem"]>;
  totalBytes: string;
  availableBytes: string;
  diskOrdinal: number;
  partitionNumber: number;
}>;
type AgentSuccess = Readonly<{ ok: true; disks: readonly ReadOnlyDisk[]; volumes: readonly AgentVolume[] }>;
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

function validVolumeBytes(value: unknown): value is string | null {
  if (value === null) return true;
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,19})$/.test(value)) return false;
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

function validDriveRoot(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z]:\\$/.test(value);
}

function validateVolume(value: unknown, expectedOrdinal: number): AgentVolume | null {
  if (!isRecord(value) || !hasExactKeys(value, [
    "ordinal", "mountPoint", "driveType", "fileSystem", "totalBytes", "availableBytes",
    "readOnly", "eligible", "eligibilityReason", "diskOrdinal", "partitionNumber"
  ])) return null;
  const reason = value.eligibilityReason;
  const mountPoint = value.mountPoint;
  const driveType = value.driveType;
  const fileSystem = value.fileSystem;
  const diskOrdinal = value.diskOrdinal;
  const partitionNumber = value.partitionNumber;
  if (value.ordinal !== expectedOrdinal ||
      !(mountPoint === null || validDriveRoot(mountPoint)) ||
      typeof driveType !== "string" || !DRIVE_TYPES.has(driveType) ||
      !(fileSystem === null || (typeof fileSystem === "string" && FILE_SYSTEMS.has(fileSystem))) ||
      !validVolumeBytes(value.totalBytes) || !validVolumeBytes(value.availableBytes) ||
      typeof value.readOnly !== "boolean" || typeof value.eligible !== "boolean" ||
      typeof reason !== "string" || !VOLUME_REASONS.has(reason as VolumeEligibilityReason) ||
      !(diskOrdinal === null || (Number.isInteger(diskOrdinal) && (diskOrdinal as number) >= 1 && (diskOrdinal as number) <= MAX_DISKS)) ||
      !(partitionNumber === null || (Number.isInteger(partitionNumber) && (partitionNumber as number) >= 1 && (partitionNumber as number) <= 128))) {
    return null;
  }
  if (value.totalBytes !== null && value.availableBytes !== null &&
      BigInt(value.availableBytes) > BigInt(value.totalBytes)) return null;
  if (value.eligible !== (reason === "eligible") || (value.eligible && (
    !validDriveRoot(mountPoint) ||
    !["fixed", "removable"].includes(driveType as string) ||
    fileSystem === null || value.readOnly || value.totalBytes === null || value.availableBytes === null ||
    BigInt(value.availableBytes) < 2n * 1024n * 1024n * 1024n ||
    diskOrdinal === null || partitionNumber === null
  )) || (!value.eligible && (diskOrdinal !== null || partitionNumber !== null))) return null;
  return value as unknown as AgentVolume;
}

/** Validate the complete agent envelope; unknown fields and stale IDs fail closed. */
export function validateAgentResponse(value: unknown, requestId: string): AgentResponse | null {
  if (!isRecord(value) || value.protocolVersion !== 2 || value.requestId !== requestId || typeof value.ok !== "boolean") {
    return null;
  }
  if (value.ok === false) {
    if (!hasExactKeys(value, ["protocolVersion", "requestId", "ok", "error"]) ||
        typeof value.error !== "string" || !AGENT_ERRORS.has(value.error)) return null;
    return { ok: false, error: value.error as AgentFailure["error"] };
  }
  if (!hasExactKeys(value, ["protocolVersion", "requestId", "ok", "disks", "volumes"]) ||
      !Array.isArray(value.disks) || value.disks.length > MAX_DISKS ||
      !Array.isArray(value.volumes) || value.volumes.length > MAX_VOLUMES) return null;
  const disks: ReadOnlyDisk[] = [];
  for (let index = 0; index < value.disks.length; index += 1) {
    const disk = validateDisk(value.disks[index], index + 1);
    if (!disk) return null;
    disks.push(disk);
  }
  const volumes: AgentVolume[] = [];
  for (let index = 0; index < value.volumes.length; index += 1) {
    const volume = validateVolume(value.volumes[index], index + 1);
    if (!volume) return null;
    volumes.push(volume);
  }
  return { ok: true, disks, volumes };
}

type InventoryError = Extract<ReadOnlyDiskInventory, { status: "error" }>["error"];

function errorResult(error: InventoryError): ReadOnlyDiskInventory {
  return { protocolVersion: 2, status: "error", error };
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
    let pendingResult: AgentRunResult | undefined;
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
    const stopAndReap = (result: AgentRunResult) => {
      if (pendingResult || settled) return;
      pendingResult = result;
      child.kill();
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
      stopAndReap({ kind: "timeout" });
    }, timeoutMs);
    child.stdout.on("data", (chunk: Buffer | string) => {
      if (pendingResult) return;
      const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      size += bytes.length;
      if (size > MAX_RESPONSE_BYTES) {
        stopAndReap({ kind: "oversize" });
      } else {
        chunks.push(bytes);
      }
    });
    child.once("error", () => {
      if (!pendingResult) pendingResult = { kind: "spawn-error" };
    });
    child.once("close", code => {
      if (settled) return;
      if (pendingResult) {
        finish(pendingResult);
        return;
      }
      if (code === AGENT_HARD_DEADLINE_EXIT_CODE) {
        finish({ kind: "timeout" });
        return;
      }
      if (code !== 0) {
        finish({ kind: "exit-error" });
        return;
      }
      finish({ kind: "response", value: readOneJsonLine(chunks) });
    });
    child.stdin.once("error", () => stopAndReap({ kind: "spawn-error" }));
    try {
      child.stdin.end(JSON.stringify({
        protocolVersion: 2,
        requestId,
        operation: "inspectPhysicalDisks"
      }) + "\n");
    } catch {
      stopAndReap({ kind: "spawn-error" });
    }
  });
}

export class ReadOnlyDiskInventoryService {
  private active: Promise<ReadOnlyDiskInventory> | undefined;
  private readonly selections = new Map<string, VolumeSelection>();
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

  /** Resolve only a token from the latest scan; no disk operation is performed. */
  resolveSelectionToken(token: string): VolumeSelection | null {
    return this.selections.get(token) ?? null;
  }

  private async run(): Promise<ReadOnlyDiskInventory> {
    this.selections.clear();
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
    const volumes: ReadOnlyVolume[] = message.volumes.map(volume => {
      const selectionToken = volume.eligible ? randomUUID() : null;
      if (selectionToken && volume.mountPoint && volume.fileSystem &&
          volume.totalBytes && volume.availableBytes && volume.diskOrdinal && volume.partitionNumber) {
        this.selections.set(selectionToken, {
          volumeOrdinal: volume.ordinal,
          mountPoint: volume.mountPoint,
          fileSystem: volume.fileSystem,
          totalBytes: volume.totalBytes,
          availableBytes: volume.availableBytes,
          diskOrdinal: volume.diskOrdinal,
          partitionNumber: volume.partitionNumber
        });
      }
      return { ...volume, selectionToken };
    });
    return {
      protocolVersion: 2,
      status: "ok",
      scannedAt: this.now().toISOString(),
      disks: message.disks,
      volumes
    };
  }
}
