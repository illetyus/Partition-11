import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ReadOnlyImageInspectionService,
  type ImageWorkerRequest,
  type ImageWorkerRunResult
} from "./imageInspection";

function directoryResponse(entries: Array<{ kind: number; size: bigint; name: Buffer }>): Buffer {
  const parts: Buffer[] = [Buffer.from("P11IMR02"), Buffer.from([0, 1])];
  const count = Buffer.alloc(4);
  count.writeUInt32LE(entries.length);
  parts.push(count);
  for (const entry of entries) {
    const fixed = Buffer.alloc(11);
    fixed[0] = entry.kind;
    fixed.writeBigUInt64LE(entry.size, 1);
    fixed.writeUInt16LE(entry.name.length, 9);
    parts.push(fixed, entry.name);
  }
  return Buffer.concat(parts);
}

function exportResponse(bytes: bigint): Buffer {
  const count = Buffer.alloc(8);
  count.writeBigUInt64LE(bytes);
  return Buffer.concat([Buffer.from("P11IMR02"), Buffer.from([0, 2]), count]);
}

function completed(stdout: Buffer): ImageWorkerRunResult {
  return { kind: "completed", stdout };
}

test("keeps host image paths in main and returns opaque directory/file tokens", async () => {
  const requests: ImageWorkerRequest[] = [];
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async (_executable, request) => {
      requests.push(request);
      return completed(directoryResponse([{ kind: 2, size: 4096n, name: Buffer.from("folder") }]));
    }
  });

  const opened = await service.openImage("D:\\private\\secret.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  assert.equal(opened.imageName, "secret.img");
  assert.equal(JSON.stringify(opened).includes("D:\\\\private"), false);
  assert.equal(opened.entries[0]?.name, "folder");
  assert.ok(opened.entries[0]?.id);
  assert.equal(requests[0]?.imagePath, "D:\\private\\secret.img");
  assert.deepEqual(requests[0]?.internalPath, Buffer.from("/"));
});

test("directory navigation uses raw path bytes held behind a main-process token", async () => {
  const requests: ImageWorkerRequest[] = [];
  const responses = [
    directoryResponse([{ kind: 2, size: 0n, name: Buffer.from([0xff, 0x64]) }]),
    directoryResponse([])
  ];
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async (_executable, request) => {
      requests.push(request);
      return completed(responses.shift()!);
    }
  });

  const opened = await service.openImage("D:\\images\\x.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  const entry = opened.entries[0];
  assert.ok(entry?.id);
  assert.equal(entry?.name.includes("�"), true);
  const listed = await service.listDirectory(opened.sessionId, entry!.id!);
  assert.equal(listed.status, "ok");
  assert.deepEqual(requests[1]?.internalPath, Buffer.from([0x2f, 0xff, 0x64]));
});

test("rejects unrecognized directory tokens without starting a worker", async () => {
  let calls = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => {
      calls += 1;
      return completed(directoryResponse([]));
    }
  });
  const result = await service.listDirectory("00000000-0000-4000-8000-000000000000", "bad-token");
  assert.deepEqual(result, { status: "error", error: "session_expired" });
  assert.equal(calls, 0);
});

test("exports only a file token and never returns the user-selected destination", async () => {
  const requests: ImageWorkerRequest[] = [];
  let call = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async (_executable, request) => {
      requests.push(request);
      call += 1;
      return completed(call === 1
        ? directoryResponse([{ kind: 1, size: 5n, name: Buffer.from("hello.txt") }])
        : exportResponse(5n));
    }
  });
  const opened = await service.openImage("D:\\images\\x.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  const file = opened.entries[0];
  assert.ok(file?.id);
  const exported = await service.exportFile(opened.sessionId, file!.id!, "D:\\exports\\hello.txt");
  assert.deepEqual(exported, { status: "exported", bytes: "5" });
  assert.deepEqual(requests[1]?.internalPath, Buffer.from("/hello.txt"));
  assert.equal(requests[1]?.destinationPath, "D:\\exports\\hello.txt");
  assert.match(requests[1]?.stagingPath ?? "", /^D:\\exports\\\.p11-[0-9a-f-]+\.partial$/);
  assert.equal(JSON.stringify(exported).includes("D:\\\\exports"), false);
});

test("does not delete a staging-path collision it did not create", async () => {
  const requests: ImageWorkerRequest[] = [];
  const removed: string[] = [];
  let call = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    stagingExists: () => false,
    removeStagingFile: filePath => removed.push(filePath),
    runWorker: async (_executable, request) => {
      requests.push(request);
      call += 1;
      if (call === 1) return completed(directoryResponse([{ kind: 1, size: 5n, name: Buffer.from("hello.txt") }]));
      return completed(Buffer.from("P11IMR02\u0006", "binary"));
    }
  });
  const opened = await service.openImage("D:\\images\\collision.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;

  assert.deepEqual(await service.exportFile(opened.sessionId, opened.entries[0]!.id, "D:\\exports\\hello.txt"), {
    status: "error",
    error: "output_exists"
  });
  assert.ok(requests[1]?.stagingPath);
  assert.deepEqual(removed, []);
});
test("directory listing is paged and continuation tokens are single use", async () => {
  const entries = Array.from({ length: 130 }, (_, index) => ({
    kind: 1,
    size: BigInt(index),
    name: Buffer.from("file-" + index + ".txt")
  }));
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => completed(directoryResponse(entries))
  });
  const opened = await service.openImage("D:\\images\\paged.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  assert.equal(opened.entries.length, 128);
  assert.ok(opened.nextPageToken);
  const page = service.nextDirectoryPage(opened.sessionId, opened.nextPageToken!);
  assert.equal(page.status, "ok");
  if (page.status !== "ok") return;
  assert.equal(page.entries.length, 2);
  assert.equal(page.entries[0]?.name, "file-128.txt");
  assert.equal(page.nextPageToken, null);
  assert.deepEqual(service.nextDirectoryPage(opened.sessionId, opened.nextPageToken!), {
    status: "error",
    error: "session_expired"
  });
});

test("rejects device and network image paths before launching a worker", async () => {
  let calls = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => {
      calls += 1;
      return completed(directoryResponse([]));
    }
  });
  const paths = [
    "\\" + "\\" + "." + "\\" + "PhysicalDrive0",
    "\\" + "\\" + "server" + "\\" + "share" + "\\" + "image.ext4",
    "C:\\images\\NUL.img"
  ];
  for (const imagePath of paths) {
    const result = await service.openImage(imagePath);
    assert.deepEqual(result, { status: "error", error: "invalid_path" });
  }
  assert.equal(calls, 0);
});

test("an oversized file cannot be exported and does not start another worker", async () => {
  let calls = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => {
      calls += 1;
      return completed(directoryResponse([{ kind: 1, size: 4n * 1024n * 1024n * 1024n + 1n, name: Buffer.from("huge.bin") }]));
    }
  });
  const opened = await service.openImage("D:\\images\\large.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  const result = await service.exportFile(opened.sessionId, opened.entries[0]!.id, "D:\\exports\\huge.bin");
  assert.deepEqual(result, { status: "error", error: "file_too_large" });
  assert.equal(calls, 1);
});

test("worker timeout invalidates the old image session and discards stale navigation", async () => {
  let calls = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => {
      calls += 1;
      return calls === 1 ? completed(directoryResponse([])) : { kind: "timeout" };
    }
  });
  const opened = await service.openImage("D:\\images\\timeout.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  assert.deepEqual(await service.listDirectory(opened.sessionId, opened.directoryId), {
    status: "error",
    error: "worker_timeout"
  });
  assert.deepEqual(await service.listDirectory(opened.sessionId, opened.directoryId), {
    status: "error",
    error: "session_expired"
  });
  assert.equal(calls, 2);
});

test("one in-flight image worker is allowed and later requests fail closed", async () => {
  let calls = 0;
  let release: ((result: ImageWorkerRunResult) => void) | undefined;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => {
      calls += 1;
      return await new Promise<ImageWorkerRunResult>(resolve => { release = resolve; });
    }
  });
  const first = service.openImage("D:\\images\\serialized.img");
  await new Promise<void>(resolve => setImmediate(resolve));
  assert.deepEqual(await service.openImage("D:\\images\\second.img"), {
    status: "error",
    error: "worker_busy"
  });
  assert.equal(calls, 1);
  release?.(completed(directoryResponse([])));
  assert.equal((await first).status, "opened");
});

test("display names neutralize terminal controls and bidirectional overrides while path bytes stay exact", async () => {
  const rawName = Buffer.from([0x61, 0x0a, 0x62, 0xe2, 0x80, 0xae, 0x63]);
  let requestPath: Buffer | undefined;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async (_executable, request) => {
      if (request.internalPath.length > 1) requestPath = request.internalPath;
      return completed(requestPath
        ? directoryResponse([])
        : directoryResponse([{ kind: 2, size: 0n, name: rawName }]));
    }
  });
  const opened = await service.openImage("D:\\images\\display.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  const entry = opened.entries[0]!;
  assert.equal(entry.name, "a�b�c");
  await service.listDirectory(opened.sessionId, entry.id);
  assert.deepEqual(requestPath, Buffer.concat([Buffer.from("/"), rawName]));
});

test("reports unclean ext4 images as requiring journal recovery", async () => {
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    runWorker: async () => completed(Buffer.from("P11IMR02\u0008", "binary"))
  });
  assert.deepEqual(await service.openImage("D:\\images\\dirty.img"), {
    status: "error",
    error: "filesystem_needs_recovery"
  });
});
test("cleans the private staging export after a worker timeout", async () => {
  const requests: ImageWorkerRequest[] = [];
  const removed: string[] = [];
  let call = 0;
  const service = new ReadOnlyImageInspectionService("agent.exe", {
    exists: () => true,
    stagingExists: () => false,
    removeStagingFile: filePath => removed.push(filePath),
    runWorker: async (_executable, request) => {
      requests.push(request);
      call += 1;
      return call === 1
        ? completed(directoryResponse([{ kind: 1, size: 5n, name: Buffer.from("hello.txt") }]))
        : { kind: "timeout" };
    }
  });
  const opened = await service.openImage("D:\\images\\timeout-export.img");
  assert.equal(opened.status, "opened");
  if (opened.status !== "opened") return;
  const result = await service.exportFile(opened.sessionId, opened.entries[0]!.id, "D:\\exports\\hello.txt");
  assert.deepEqual(result, { status: "error", error: "worker_timeout" });
  assert.equal(requests[1]?.stagingPath, removed[0]);
  assert.notEqual(requests[1]?.stagingPath, requests[1]?.destinationPath);
});
