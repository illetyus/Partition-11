import { randomUUID } from "node:crypto";
import { open, mkdtemp, lstat, realpath, unlink, rmdir } from "node:fs/promises";
import path from "node:path";

/**
 * P2.5B.1 scratch-file lease.
 *
 * This low-level helper is used only in isolated automated tests. Production
 * volume authorization (P2 stable volume ID / protected-volume checks),
 * native Win32 handle revalidation and Job Objects are not yet implemented.
 * It MUST NOT be exported via the renderer/preload IPC bridge.
 */
const MAX_CI_FILE_BYTES = 64 * 1024 * 1024;
const MIN_CI_FILE_BYTES = 4 * 1024 * 1024;
const RUN_DIR_PREFIX = "p11-bench-";

type FileIdentity = Readonly<{
  dev: number;
  ino: number;
  birthtimeMs: number;
}>;
const identity = (stat: { dev: number; ino: number; birthtimeMs: number }): FileIdentity =>
  ({ dev: stat.dev, ino: stat.ino, birthtimeMs: stat.birthtimeMs });

function sameIdentity(a: FileIdentity, b: FileIdentity): boolean {
  return a.dev === b.dev && a.ino === b.ino && a.birthtimeMs === b.birthtimeMs;
}

function assertSafeFileSize(bytes: number): void {
  if (!Number.isSafeInteger(bytes) || bytes < MIN_CI_FILE_BYTES ||
      bytes > MAX_CI_FILE_BYTES || bytes % 4096 !== 0) {
    throw new Error("CI benchmark scratch size outside safe bounds");
  }
}

export class OwnedScratchLease {
  readonly filePath: string;
  readonly fileBytes: number;
  private readonly directory: string;
  private readonly directoryIdentity: FileIdentity;
  private readonly fileIdentity: FileIdentity;
  private cleaned = false;

  private constructor(filePath: string, fileBytes: number, directory: string,
                      directoryIdentity: FileIdentity, fileIdentity: FileIdentity) {
    this.filePath = filePath;
    this.fileBytes = fileBytes;
    this.directory = directory;
    this.directoryIdentity = directoryIdentity;
    this.fileIdentity = fileIdentity;
  }

  static async createInsideTrustedTestRoot(testRoot: string, bytes: number): Promise<OwnedScratchLease> {
    assertSafeFileSize(bytes);
    if (!path.isAbsolute(testRoot)) throw new Error("Scratch root must be absolute");
    const rootMetadata = await lstat(testRoot);
    if (!rootMetadata.isDirectory() || rootMetadata.isSymbolicLink()) {
      throw new Error("Scratch root is not a real directory");
    }
    const canonicalRoot = await realpath(testRoot);
    if (canonicalRoot !== testRoot) {
      throw new Error("Scratch root must be canonical");
    }
    const directory = await mkdtemp(path.join(canonicalRoot, RUN_DIR_PREFIX));
    let filePath: string | undefined;
    try {
      const dirMetadata = await lstat(directory);
      if (!dirMetadata.isDirectory() || dirMetadata.isSymbolicLink()) {
        throw new Error("Unsafe benchmark scratch directory");
      }
      filePath = path.join(directory, "p11-" + randomUUID() + ".bin");
      const handle = await open(filePath, "wx+", 0o600);
      let fileIdentity: FileIdentity;
      try {
        await handle.truncate(bytes);
        await handle.sync();
        const stat = await handle.stat();
        if (!stat.isFile() || stat.size !== bytes || stat.nlink !== 1) {
          throw new Error("Scratch file failed exclusive-ownership check");
        }
        fileIdentity = identity(stat);
      } finally {
        await handle.close();
      }
      const lease = new OwnedScratchLease(
        filePath, bytes, directory, identity(dirMetadata), fileIdentity
      );
      await lease.verify();
      return lease;
    } catch (e) {
      // Only clean the exclusively created file and directory, never a glob.
      // On an unexpected identity issue the helper leaves evidence intact.
      if (filePath) {
        try {
          const stat = await lstat(filePath);
          if (stat.isFile() && !stat.isSymbolicLink()) await unlink(filePath);
        } catch { /* The file might never have been created. */ }
      }
      try { await rmdir(directory); } catch { /* Fail closed on unexpected entries. */ }
      throw e;
    }
  }

  async verify(): Promise<void> {
    if (this.cleaned) throw new Error("Scratch file already cleaned");
    const dir = await lstat(this.directory);
    const file = await lstat(this.filePath);
    if (!dir.isDirectory() || dir.isSymbolicLink() ||
        !sameIdentity(this.directoryIdentity, identity(dir)) ||
        !file.isFile() || file.isSymbolicLink() || file.nlink !== 1 ||
        !sameIdentity(this.fileIdentity, identity(file)) ||
        file.size !== this.fileBytes) {
      throw new Error("Scratch file or directory identity changed");
    }
  }

  async cleanup(): Promise<void> {
    if (this.cleaned) return;
    await this.verify(); // Refuse deletion if target was replaced/tampered.
    await unlink(this.filePath);
    await rmdir(this.directory); // Fails if unrecognized extra files exist.
    this.cleaned = true;
  }
}
