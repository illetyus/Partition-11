import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";
import test from "node:test";
import type { ChildProcess } from "node:child_process";
import { ReadOnlyDiskInventoryService, validateAgentResponse } from "./readOnlyDiskInventory";

const disk = {
  ordinal: 1,
  availability: "ready",
  capacityBytes: "1000000000",
  logicalSectorSize: 512,
  physicalSectorSize: 4096,
  busType: "usb",
  partitionStyle: "gpt",
  partitionCount: 2
} as const;

const identityHash = "ab".repeat(32);
const agentDisk = { ...disk, identityHash } as const;

const volume = {
  ordinal: 1,
  mountPoint: "E:\\",
  driveType: "fixed",
  fileSystem: "NTFS",
  totalBytes: "10737418240",
  availableBytes: "4294967296",
  readOnly: false,
  eligible: true,
  eligibilityReason: "eligible",
  diskOrdinal: 1,
  partitionNumber: 1
} as const;

function success(requestId: string, disks: unknown[] = [agentDisk], volumes: unknown[] = [volume]) {
  return { protocolVersion: 3, requestId, ok: true, disks, volumes };
}

test("agent response accepts the bounded public allowlist and rejects private or extra fields", () => {
  assert.deepEqual(validateAgentResponse(success("req-1", [agentDisk]), "req-1"), { ok: true, disks: [agentDisk], volumes: [volume] });
  assert.equal(validateAgentResponse({ ...success("req-1"), serial: "secret" }, "req-1"), null);
  assert.equal(validateAgentResponse(success("stale"), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, deviceNumber: 8 }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, capacityBytes: "01" }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, logicalSectorSize: 513 }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, partitionCount: 129 }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, ordinal: 2 }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [disk]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, identityHash: "ABC" }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, identityHash: `${identityHash}\n` }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{ ...agentDisk, identityHash: null }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [{
    ...agentDisk,
    availability: "device_unavailable",
    capacityBytes: null,
    logicalSectorSize: null,
    physicalSectorSize: null,
    busType: null,
    partitionStyle: null,
    partitionCount: null
  }]), "req-1"), null);
  assert.deepEqual(
    validateAgentResponse(success("req-1", [{ ...agentDisk, identityHash: null }], [{
      ...volume,
      eligible: false,
      eligibilityReason: "identity_unavailable",
      diskOrdinal: null,
      partitionNumber: null
    }]), "req-1"),
    { ok: true, disks: [{ ...agentDisk, identityHash: null }], volumes: [{
      ...volume,
      eligible: false,
      eligibilityReason: "identity_unavailable",
      diskOrdinal: null,
      partitionNumber: null
    }] }
  );
  assert.equal(validateAgentResponse(success("req-1", [disk], [{ ...volume, volumeGuidPath: "\\\\?\\Volume{secret}\\" }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [disk], [{ ...volume, eligibilityReason: "system_volume" }]), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [disk], [{ ...volume, mountPoint: "C:\\Users\\name" }]), "req-1"), null);
});

test("agent response rejects oversized inventory and unrecognized error codes", () => {
  assert.equal(validateAgentResponse(success("req-1", Array(257).fill(disk)), "req-1"), null);
  assert.equal(validateAgentResponse(success("req-1", [disk], Array(257).fill(volume)), "req-1"), null);
  assert.equal(validateAgentResponse({ protocolVersion: 1, requestId: "req-1", ok: false, error: "C:\\raw-path" }, "req-1"), null);
  assert.deepEqual(validateAgentResponse({ protocolVersion: 3, requestId: "req-1", ok: false, error: "scan_timeout" }, "req-1"), { ok: false, error: "scan_timeout" });
});

class FakeChild extends EventEmitter {
  readonly stdin = new PassThrough();
  readonly stdout = new PassThrough();
  killed = false;

  constructor(
    onRequest?: (request: Record<string, unknown>, child: FakeChild) => void,
    private readonly onKill?: (child: FakeChild) => void
  ) {
    super();
    let input = "";
    this.stdin.setEncoding("utf8");
    this.stdin.on("data", chunk => { input += String(chunk); });
    this.stdin.on("finish", () => {
      if (!onRequest) return;
      try { onRequest(JSON.parse(input) as Record<string, unknown>, this); } catch { /* test fixture */ }
    });
  }

  kill(): boolean {
    this.killed = true;
    this.onKill?.(this);
    return true;
  }
}

function asChild(child: FakeChild): ChildProcess {
  return child as unknown as ChildProcess;
}

test("missing executable returns a safe error without spawning", async () => {
  let spawned = false;
  const service = new ReadOnlyDiskInventoryService("missing-agent.exe", {
    exists: () => false,
    spawn: () => { spawned = true; throw new Error("must not start"); }
  });
  assert.deepEqual(await service.scan(), { protocolVersion: 2, status: "error", error: "agent_unavailable" });
  assert.equal(spawned, false);
});

test("timeout kills the one-shot agent and discards any later output", async () => {
  let child: FakeChild | undefined;
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    timeoutMs: 10,
    spawn: () => {
      child = new FakeChild(undefined, process => {
        setImmediate(() => process.emit("close", null, "SIGTERM"));
      });
      return asChild(child);
    }
  });
  assert.deepEqual(await service.scan(), { protocolVersion: 2, status: "error", error: "timeout" });
  assert.equal(child?.killed, true);
});

test("agent hard-deadline exit is reported as a timeout", async () => {
  const child = new FakeChild();
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    spawn: () => {
      setImmediate(() => child.emit("close", 124, null));
      return asChild(child);
    }
  });
  assert.deepEqual(await service.scan(), { protocolVersion: 2, status: "error", error: "timeout" });
});

test("timeout keeps the inventory slot occupied until the killed child closes", async () => {
  let starts = 0;
  const children: FakeChild[] = [];
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    timeoutMs: 10,
    spawn: () => {
      starts += 1;
      const child = new FakeChild();
      children.push(child);
      return asChild(child);
    }
  });
  const first = service.scan();
  await new Promise(resolve => setTimeout(resolve, 30));
  const second = service.scan();
  const startsBeforeClose = starts;
  for (const child of children) child.emit("close", null, "SIGTERM");
  await Promise.all([first, second]);
  assert.equal(second, first);
  assert.equal(startsBeforeClose, 1);
});

test("stdin pipe errors kill the agent and resolve as a safe failure", async () => {
  const child = new FakeChild();
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    spawn: () => asChild(child)
  });
  const scan = service.scan();
  child.stdin.emit("error", Object.assign(new Error("broken pipe"), { code: "EPIPE" }));
  assert.equal(child.killed, true);
  child.emit("close", null, "SIGTERM");
  assert.deepEqual(await scan, { protocolVersion: 2, status: "error", error: "agent_unavailable" });
});

test("only one inventory process runs at a time and a mismatched request ID is discarded", async () => {
  let starts = 0;
  const child = new FakeChild((request, process) => {
    const response = JSON.stringify(success("wrong-id")) + "\n";
    process.stdout.end(response);
    setImmediate(() => process.emit("close", 0, null));
  });
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    spawn: () => { starts += 1; return asChild(child); }
  });
  const first = service.scan();
  const second = service.scan();
  assert.equal(first, second);
  assert.deepEqual(await first, { protocolVersion: 2, status: "error", error: "invalid_response" });
  assert.equal(starts, 1);
});

test("valid agent response is timestamped only after a clean exit", async () => {
  let agentRequest: Record<string, unknown> | undefined;
  const child = new FakeChild((request, process) => {
    agentRequest = request;
    process.stdout.end(JSON.stringify(success(String(request.requestId))) + "\n");
    setImmediate(() => process.emit("close", 0, null));
  });
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    spawn: () => asChild(child),
    now: () => new Date("2026-10-19T12:00:00.000Z")
  });
  const result = await service.scan();
  assert.equal(result.status, "ok");
  if (result.status !== "ok") throw new Error("expected successful inventory");
  assert.deepEqual(result, {
    protocolVersion: 2,
    status: "ok",
    scannedAt: "2026-10-19T12:00:00.000Z",
    disks: [disk],
    volumes: [{ ...volume, selectionToken: result.volumes[0]?.selectionToken }]
  });
  assert.equal(agentRequest?.protocolVersion, 3);
  assert.equal(agentRequest?.operation, "inspectPhysicalDisks");
  assert.equal(typeof agentRequest?.identityKey, "string");
  assert.match(agentRequest?.identityKey as string, /^[0-9a-f]{64}$/);
  assert.equal(JSON.stringify(result).includes(identityHash), false);
  const token = result.volumes[0]?.selectionToken;
  assert.ok(token);
  const resolved = {
    volumeOrdinal: 1,
    mountPoint: "E:\\",
    fileSystem: "NTFS",
    totalBytes: "10737418240",
    availableBytes: "4294967296",
    diskOrdinal: 1,
    partitionNumber: 1,
    identityHash
  };
  assert.deepEqual(service.resolveSelectionToken(token), resolved);
  const consumed = service.consumeSelectionToken(token);
  assert.ok(consumed);
  assert.deepEqual(consumed.selection, resolved);
  assert.equal(consumed.identityKey.toString("hex"), agentRequest?.identityKey);
  assert.equal(consumed.identityKey.length, 32);
  assert.equal(service.resolveSelectionToken(token), null);
  assert.equal(service.consumeSelectionToken(token), null);
});

test("selection tokens expire after one minute even without a later inventory scan", async () => {
  let now = new Date("2026-10-19T12:00:00.000Z");
  const child = new FakeChild((request, process) => {
    process.stdout.end(JSON.stringify(success(String(request.requestId))) + "\n");
    setImmediate(() => process.emit("close", 0, null));
  });
  const service = new ReadOnlyDiskInventoryService("agent.exe", {
    exists: () => true,
    spawn: () => asChild(child),
    now: () => now
  });
  const result = await service.scan();
  assert.equal(result.status, "ok");
  if (result.status !== "ok") throw new Error("expected successful inventory");
  const token = result.volumes[0]?.selectionToken;
  assert.ok(token);
  now = new Date(now.getTime() + 60_001);
  assert.equal(service.consumeSelectionToken(token), null);
  assert.equal(service.resolveSelectionToken(token), null);
});
