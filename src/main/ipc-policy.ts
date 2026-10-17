import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Default-deny frame origin policy. The expected absolute file path prevents
 * arbitrary local file:// content from invoking main-process APIs.
 */
export function isAllowedFrame(
  actualFrameUrl: string | undefined,
  expectedHtmlPath: string,
  developmentUrl?: string
): boolean {
  if (!actualFrameUrl) return false;

  try {
    const actual = new URL(actualFrameUrl);
    if (actual.search || actual.hash) return false;

    if (developmentUrl) {
      const expectedDev = new URL(developmentUrl);
      if (
        actual.protocol === "http:" &&
        actual.hostname === "127.0.0.1" &&
        actual.origin === expectedDev.origin &&
        actual.pathname === "/"
      ) {
        return true;
      }
    }

    if (actual.protocol !== "file:") return false;

    return (
      path.normalize(fileURLToPath(actual)) ===
      path.normalize(path.resolve(expectedHtmlPath))
    );
  } catch {
    return false;
  }
}
