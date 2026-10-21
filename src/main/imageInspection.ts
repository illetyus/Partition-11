import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, unlinkSync } from "node:fs";
import path from "node:path";

const REQUEST_MAGIC = Buffer.from("P11IMG02");
const RESPONSE_MAGIC = Buffer.from("P11IMR02");
const MAX_HOST_PATH_BYTES = 32 * 1024;
const MAX_INTERNAL_PATH_BYTES = 4096;
const MAX_DIRECTORY_ENTRIES = 16_384;
const MAX_PAGE_ENTRIES = 128;
const MAX_ACTIVE_SESSIONS = 2;
const MAX_SESSION_ENTRIES = 4_096;
const MAX_CURSORS_PER_SESSION = 4;
const MAX_RESPONSE_BYTES = 8 * 1024 * 1024;
const SESSION_TTL_MS = 15 * 60 * 1000;
const LIST_TIMEOUT_MS = 20_000;
const EXPORT_TIMEOUT_MS = 10 * 60 * 1000;
const MAX_EXPORT_BYTES = 4n * 1024n * 1024n * 1024n;

export type ImageEntryKind = "file" | "directory" | "symlink" | "other";
export type ImageInspectionError =
  | "agent_unavailable"
  | "worker_busy"
  | "worker_timeout"
  | "invalid_response"
  | "invalid_image"
  | "unsupported_filesystem"
  | "filesystem_needs_recovery"
  | "path_unavailable"
  | "file_too_large"
  | "output_exists"
  | "output_failed"
  | "session_expired"
  | "session_limit"
  | "too_many_entries"
  | "invalid_path";

export type ImageEntry = Readonly<{
  id: string;
  name: string;
  kind: ImageEntryKind;
  sizeBytes: string;
}>;

export type ImageOpenResult =
  | Readonly<{
      status: "opened";
      sessionId: string;
      imageName: string;
      directoryId: string;
      entries: readonly ImageEntry[];
      nextPageToken: string | null;
    }>
  | Readonly<{ status: "cancelled" }>
  | Readonly<{ status: "error"; error: ImageInspectionError }>;

export type ImageDirectoryResult =
  | Readonly<{
      status: "ok";
      entries: readonly ImageEntry[];
      nextPageToken: string | null;
    }>
  | Readonly<{ status: "error"; error: ImageInspectionError }>;

export type ImageExportResult =
  | Readonly<{ status: "exported"; bytes: string }>
  | Readonly<{ status: "error"; error: ImageInspectionError }>;

export type ImageWorkerRequest = Readonly<{
  imagePath: string;
  internalPath: Buffer;
  destinationPath?: string;
  stagingPath?: string;
  timeoutMs: number;
}>;

export type ImageWorkerRunResult =
  | Readonly<{ kind: "completed"; stdout: Buffer }>
  | Readonly<{ kind: "timeout" }>
  | Readonly<{ kind: "failed" }>;

type WorkerEntry = Readonly<{
  kind: ImageEntryKind;
  sizeBytes: bigint;
  name: Buffer;
}>;

type WorkerReply =
  | Readonly<{ kind: "directory"; entries: readonly WorkerEntry[] }>
  | Readonly<{ kind: "exported"; bytes: bigint }>
  | Readonly<{ kind: "error"; error: ImageInspectionError }>;

type SessionPath = Readonly<{ pathBytes: Buffer; kind: "directory" | "file" | "blocked"; sizeBytes: bigint; displayName: string }>;
type Cursor = { entries: readonly WorkerEntry[]; index: number; pathBytes: Buffer };
type Session = {
  imagePath: string;
  expiresAt: number;
  paths: Map<string, SessionPath>;
  cursors: Map<string, Cursor>;
  exposedEntries: number;
};

type ServiceOptions = Readonly<{
  exists?: (filePath: string) => boolean;
  stagingExists?: (filePath: string) => boolean;
  now?: () => number;
  removeStagingFile?: (filePath: string) => void;
  runWorker?: (executable: string, request: ImageWorkerRequest) => Promise<ImageWorkerRunResult>;
  token?: () => string;
}>;

function isSafeLocalWindowsPath(value: string): boolean {
  if (value.length === 0 || Buffer.byteLength(value, "utf8") > MAX_HOST_PATH_BYTES || value.includes("\0")) return false;
  if (!/^[A-Za-z]:\\/.test(value) || value.startsWith("\\\\") || value.includes("/")) return false;
  const parts = value.slice(3).split("\\");
  for (const part of parts) {
    if (part === "." || part === "..") return false;
    const stem = part.replace(/[. ]+$/g, "").split(".")[0]?.toUpperCase() ?? "";
    if (/^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/.test(stem)) return false;
  }
  return true;
}

function makeStagingPath(destinationPath: string, exists: (filePath: string) => boolean): string | null {
  const parent = path.win32.dirname(destinationPath);
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const candidate = path.win32.join(parent, `.p11-${randomUUID()}.partial`);
    if (!exists(candidate)) return candidate;
  }
  return null;
}

function sanitizeVisibleName(value: string): string {
  return Array.from(value).map(character => {
    const code = character.codePointAt(0) ?? 0;
    const control = code < 0x20 || (code >= 0x7f && code <= 0x9f);
    const bidi = (code >= 0x202a && code <= 0x202e) || (code >= 0x2066 && code <= 0x2069) ||
      code === 0x200e || code === 0x200f || code === 0x061c;
    return control || bidi ? String.fromCharCode(0xfffd) : character;
  }).slice(0, 255).join("");
}
function validateInternalPath(pathBytes: Buffer): boolean {
  return pathBytes.length > 0 && pathBytes.length <= MAX_INTERNAL_PATH_BYTES &&
    pathBytes[0] === 0x2f && !pathBytes.includes(0);
}

function encodeRequest(request: ImageWorkerRequest): Buffer {
  const image = Buffer.from(request.imagePath, "utf8");
  const destination = request.destinationPath ? Buffer.from(request.destinationPath, "utf8") : Buffer.alloc(0);
  const staging = request.stagingPath ? Buffer.from(request.stagingPath, "utf8") : Buffer.alloc(0);
  const operation = request.destinationPath || request.stagingPath ? 2 : 1;
  const fields = [image, request.internalPath, destination, staging];
  const lengths = fields.map(field => {
    const length = Buffer.alloc(4);
    length.writeUInt32LE(field.length);
    return length;
  });
  return Buffer.concat([REQUEST_MAGIC, Buffer.from([operation]), lengths[0]!, image,
    lengths[1]!, request.internalPath, lengths[2]!, destination, lengths[3]!, staging]);
}

function decodeReply(bytes: Buffer): WorkerReply {
  if (bytes.length < 9 || !bytes.subarray(0, 8).equals(RESPONSE_MAGIC)) {
    return { kind: "error", error: "invalid_response" };
  }
  const status = bytes[8]!;
  const statusErrors: Readonly<Record<number, ImageInspectionError>> = {
    1: "invalid_response",
    2: "invalid_image",
    3: "unsupported_filesystem",
    4: "path_unavailable",
    5: "file_too_large",
    6: "output_exists",
    7: "output_failed",
    8: "filesystem_needs_recovery"
  };
  if (status !== 0) {
    return bytes.length === 9
      ? { kind: "error", error: statusErrors[status] ?? "invalid_response" }
      : { kind: "error", error: "invalid_response" };
  }
  if (bytes.length < 10) return { kind: "error", error: "invalid_response" };
  const operation = bytes[9]!;
  if (operation === 2) {
    if (bytes.length !== 18) return { kind: "error", error: "invalid_response" };
    return { kind: "exported", bytes: bytes.readBigUInt64LE(10) };
  }
  if (operation !== 1 || bytes.length < 14) return { kind: "error", error: "invalid_response" };

  const count = bytes.readUInt32LE(10);
  if (count > MAX_DIRECTORY_ENTRIES) return { kind: "error", error: "invalid_response" };
  let offset = 14;
  const entries: WorkerEntry[] = [];
  for (let index = 0; index < count; index += 1) {
    if (offset + 11 > bytes.length) return { kind: "error", error: "invalid_response" };
    const kindCode = bytes[offset]!;
    const kind: ImageEntryKind | undefined =
      kindCode === 1 ? "file" : kindCode === 2 ? "directory" : kindCode === 3 ? "symlink" : kindCode === 4 ? "other" : undefined;
    const sizeBytes = bytes.readBigUInt64LE(offset + 1);
    const nameLength = bytes.readUInt16LE(offset + 9);
    offset += 11;
    if (!kind || nameLength < 1 || nameLength > 255 || offset + nameLength > bytes.length) {
      return { kind: "error", error: "invalid_response" };
    }
    const name = Buffer.from(bytes.subarray(offset, offset + nameLength));
    if (name.includes(0) || name.includes(0x2f) || name.equals(Buffer.from(".")) || name.equals(Buffer.from(".."))) {
      return { kind: "error", error: "invalid_response" };
    }
    entries.push({ kind, sizeBytes, name });
    offset += nameLength;
  }
  if (offset !== bytes.length) return { kind: "error", error: "invalid_response" };
  return { kind: "directory", entries };
}

function runImageWorker(executable: string, request: ImageWorkerRequest): Promise<ImageWorkerRunResult> {
  return new Promise(resolve => {
    let child: ReturnType<typeof spawn>;
    try {
      child = spawn(executable, ["image-worker"], {
        shell: false,
        windowsHide: true,
        stdio: ["pipe", "pipe", "ignore"]
      });
    } catch {
      resolve({ kind: "failed" });
      return;
    }

    const stdout = child.stdout;
    const stdin = child.stdin;
    if (!stdout || !stdin) {
      child.kill();
      resolve({ kind: "failed" });
      return;
    }
    let settled = false;
    let timedOut = false;
    let exceededLimit = false;
    let stdoutBytes = 0;
    const stdoutChunks: Buffer[] = [];
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, request.timeoutMs);
    timer.unref();

    const finish = (result: ImageWorkerRunResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(result);
    };

    stdout.on("data", (chunk: Buffer) => {
      stdoutBytes += chunk.length;
      if (stdoutBytes > MAX_RESPONSE_BYTES) {
        exceededLimit = true;
        child.kill();
        return;
      }
      stdoutChunks.push(Buffer.from(chunk));
    });
    let spawnFailed = false;
    child.once("error", () => { spawnFailed = true; });
    child.once("close", code => {
      if (timedOut) finish({ kind: "timeout" });
      else if (exceededLimit || spawnFailed || code !== 0) finish({ kind: "failed" });
      else finish({ kind: "completed", stdout: Buffer.concat(stdoutChunks, stdoutBytes) });
    });
    stdin.once("error", () => child.kill());
    stdin.end(encodeRequest(request));
  });
}

export class ReadOnlyImageInspectionService {
  private readonly sessions = new Map<string, Session>();
  private readonly exists: (filePath: string) => boolean;
  private readonly stagingExists: (filePath: string) => boolean;
  private readonly removeStagingFile: (filePath: string) => void;
  private readonly now: () => number;
  private readonly runWorker: (executable: string, request: ImageWorkerRequest) => Promise<ImageWorkerRunResult>;
  private readonly token: () => string;
  private workerRunning = false;

  constructor(private readonly executable: string, options: ServiceOptions = {}) {
    this.exists = options.exists ?? existsSync;
    this.stagingExists = options.stagingExists ?? existsSync;
    this.removeStagingFile = options.removeStagingFile ?? (filePath => {
      try { unlinkSync(filePath); } catch { /* no staged output remains */ }
    });
    this.now = options.now ?? Date.now;
    this.runWorker = options.runWorker ?? runImageWorker;
    this.token = options.token ?? randomUUID;
  }

  async openImage(imagePath: string): Promise<ImageOpenResult> {
    this.pruneExpired();
    if (this.sessions.size >= MAX_ACTIVE_SESSIONS) return { status: "error", error: "session_limit" };
    if (!isSafeLocalWindowsPath(imagePath)) return { status: "error", error: "invalid_path" };
    try {
      if (!this.exists(imagePath)) return { status: "error", error: "invalid_image" };
    } catch {
      return { status: "error", error: "invalid_image" };
    }

    const reply = await this.request({ imagePath, internalPath: Buffer.from("/"), timeoutMs: LIST_TIMEOUT_MS });
    if (reply.kind !== "directory") return { status: "error", error: reply.kind === "error" ? reply.error : "invalid_response" };

    const sessionId = this.token();
    const directoryId = this.token();
    const session: Session = {
      imagePath,
      expiresAt: this.now() + SESSION_TTL_MS,
      paths: new Map([[directoryId, { pathBytes: Buffer.from("/"), kind: "directory", sizeBytes: 0n, displayName: "" }]]),
      cursors: new Map(),
      exposedEntries: 0
    };
    this.sessions.set(sessionId, session);
    const page = this.createPage(session, reply.entries, Buffer.from("/"));
    if (page.status === "error") {
      this.sessions.delete(sessionId);
      return page;
    }
    return {
      status: "opened",
      sessionId,
      imageName: sanitizeVisibleName(path.win32.basename(imagePath)),
      directoryId,
      entries: page.entries,
      nextPageToken: page.nextPageToken
    };
  }

  async listDirectory(sessionId: string, directoryId: string): Promise<ImageDirectoryResult> {
    const session = this.getSession(sessionId);
    const target = session?.paths.get(directoryId);
    if (!session || !target || target.kind !== "directory") {
      return { status: "error", error: "session_expired" };
    }
    const reply = await this.request({
      imagePath: session.imagePath,
      internalPath: target.pathBytes,
      timeoutMs: LIST_TIMEOUT_MS
    });
    if (reply.kind !== "directory") {
      this.sessions.delete(sessionId);
      return { status: "error", error: reply.kind === "error" ? reply.error : "invalid_response" };
    }
    const page = this.createPage(session, reply.entries, target.pathBytes);
    if (page.status === "error") this.sessions.delete(sessionId);
    return page;
  }

  nextDirectoryPage(sessionId: string, pageToken: string): ImageDirectoryResult {
    const session = this.getSession(sessionId);
    const cursor = session?.cursors.get(pageToken);
    if (!session || !cursor) return { status: "error", error: "session_expired" };
    session.cursors.delete(pageToken);
    const start = cursor.index;
    const end = Math.min(start + MAX_PAGE_ENTRIES, cursor.entries.length);
    const entries = this.materializeEntries(session, cursor.entries.slice(start, end), cursor.pathBytes);
    if (!entries) {
      this.sessions.delete(sessionId);
      return { status: "error", error: "too_many_entries" };
    }
    const next = end < cursor.entries.length
      ? this.createCursor(session, cursor.entries, end, cursor.pathBytes)
      : null;
    if (end < cursor.entries.length && next === null) {
      this.sessions.delete(sessionId);
      return { status: "error", error: "too_many_entries" };
    }
    return { status: "ok", entries, nextPageToken: next };
  }

  async exportFile(sessionId: string, fileId: string, destinationPath: string): Promise<ImageExportResult> {
    const session = this.getSession(sessionId);
    const target = session?.paths.get(fileId);
    if (!session || !target || target.kind !== "file") {
      return { status: "error", error: "session_expired" };
    }
    if (target.sizeBytes > MAX_EXPORT_BYTES) return { status: "error", error: "file_too_large" };
    if (!isSafeLocalWindowsPath(destinationPath)) return { status: "error", error: "invalid_path" };

    const stagingPath = makeStagingPath(destinationPath, this.stagingExists);
    if (!stagingPath) return { status: "error", error: "output_failed" };
    if (!isSafeLocalWindowsPath(stagingPath)) return { status: "error", error: "invalid_path" };

    let reply: WorkerReply;
    try {
      reply = await this.request({
        imagePath: session.imagePath,
        internalPath: target.pathBytes,
        destinationPath,
        stagingPath,
        timeoutMs: EXPORT_TIMEOUT_MS
      });
    } catch {
      try { this.removeStagingFile(stagingPath); } catch { /* cleanup is best effort after the child closes */ }
      return { status: "error", error: "agent_unavailable" };
    }
    if (reply.kind === "error" && reply.error === "output_exists") {
      return { status: "error", error: "output_exists" };
    }
    try { this.removeStagingFile(stagingPath); } catch { /* cleanup is best effort after the child closes */ }
    if (reply.kind === "exported" && reply.bytes <= MAX_EXPORT_BYTES) {
      return { status: "exported", bytes: reply.bytes.toString() };
    }
    return { status: "error", error: reply.kind === "error" ? reply.error : "invalid_response" };
  }

  suggestedExportName(sessionId: string, fileId: string): string | null {
    const target = this.getSession(sessionId)?.paths.get(fileId);
    if (!target || target.kind !== "file") return null;
    const cleaned = Array.from(target.displayName).map(character => {
      const code = character.charCodeAt(0);
      const forbidden = character === "<" || character === ">" || character === ":" ||
        character === "\"" || character === "/" || character === "|" || character === "?" ||
        character === "*" || code === 92 || code < 32;
      return forbidden ? "_" : character;
    }).join("").replace(/[. ]+$/g, "").slice(0, 180);
    return cleaned.length > 0 ? cleaned : "export.bin";
  }

  closeSession(sessionId: string): void {
    this.sessions.delete(sessionId);
  }

  closeAll(): void {
    this.sessions.clear();
  }

  private async request(request: ImageWorkerRequest): Promise<WorkerReply> {
    if (this.workerRunning) return { kind: "error", error: "worker_busy" };
    this.workerRunning = true;
    let result: ImageWorkerRunResult;
    try {
      result = await this.runWorker(this.executable, request);
    } catch {
      return { kind: "error", error: "agent_unavailable" };
    } finally {
      this.workerRunning = false;
    }
    if (result.kind === "timeout") return { kind: "error", error: "worker_timeout" };
    if (result.kind === "failed") return { kind: "error", error: "agent_unavailable" };
    if (result.stdout.length > MAX_RESPONSE_BYTES) return { kind: "error", error: "invalid_response" };
    return decodeReply(result.stdout);
  }

  private getSession(sessionId: string): Session | undefined {
    this.pruneExpired();
    const session = this.sessions.get(sessionId);
    if (session) session.expiresAt = this.now() + SESSION_TTL_MS;
    return session;
  }

  private pruneExpired(): void {
    const now = this.now();
    for (const [id, session] of this.sessions) {
      if (session.expiresAt <= now) this.sessions.delete(id);
    }
  }

  private createPage(session: Session, entries: readonly WorkerEntry[], parentPath: Buffer): ImageDirectoryResult {
    const end = Math.min(entries.length, MAX_PAGE_ENTRIES);
    const materialized = this.materializeEntries(session, entries.slice(0, end), parentPath);
    if (!materialized) return { status: "error", error: "too_many_entries" };
    const next = end < entries.length ? this.createCursor(session, entries, end, parentPath) : null;
    if (end < entries.length && next === null) return { status: "error", error: "too_many_entries" };
    return { status: "ok", entries: materialized, nextPageToken: next };
  }

  private materializeEntries(session: Session, entries: readonly WorkerEntry[], parentPath: Buffer): readonly ImageEntry[] | null {
    if (session.exposedEntries + entries.length > MAX_SESSION_ENTRIES) return null;
    const result: ImageEntry[] = [];
    for (const entry of entries) {
      if (entry.name.length < 1 || entry.name.length > 255 || entry.name.includes(0) ||
          entry.name.includes(0x2f) || entry.name.equals(Buffer.from(".")) || entry.name.equals(Buffer.from(".."))) {
        return null;
      }
      const id = this.token();
      const separator = parentPath.equals(Buffer.from("/")) ? Buffer.alloc(0) : Buffer.from("/");
      const fullPath = Buffer.concat([parentPath, separator, entry.name]);
      if (!validateInternalPath(fullPath)) return null;
      const kind = entry.kind === "directory" ? "directory"
        : entry.kind === "file" ? "file" : "blocked";
      session.paths.set(id, { pathBytes: fullPath, kind, sizeBytes: entry.sizeBytes, displayName: sanitizeVisibleName(entry.name.toString("utf8")) });
      result.push({
        id,
        name: sanitizeVisibleName(entry.name.toString("utf8")),
        kind: entry.kind,
        sizeBytes: entry.sizeBytes.toString()
      });
    }
    session.exposedEntries += entries.length;
    return result;
  }

  private createCursor(session: Session, entries: readonly WorkerEntry[], index: number, pathBytes: Buffer): string | null {
    if (session.cursors.size >= MAX_CURSORS_PER_SESSION) return null;
    const token = this.token();
    session.cursors.set(token, { entries, index, pathBytes });
    return token;
  }
}

export function encodeImageWorkerRequestForTest(request: ImageWorkerRequest): Buffer {
  return encodeRequest(request);
}
