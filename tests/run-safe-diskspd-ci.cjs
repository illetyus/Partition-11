"use strict";
/**
 * P2.5B CI-ONLY integration: official SHA256-verified DiskSpd binary
 * (selected by the workflow) and a newly created application-owned file.
 * No module here is reachable through Electron preload/renderer IPC.
 */
const path = require("node:path");
const fs = require("node:fs/promises");
const { spawn } = require("node:child_process");
const { OwnedScratchLease } = require("../dist/main/main/benchmarkScratch.js");
const { buildDiskSpdFileArgs } = require("../dist/main/main/benchmarkPolicy.js");
const { parseDiskSpdXml } = require("../dist/main/main/benchmarkXml.js");

const MAX_XML_BYTES = 1024 * 1024;
const MAX_STDERR_BYTES = 16 * 1024;

async function runDiskSpd(exe, args) {
  return await new Promise((resolve, reject) => {
    const child = spawn(exe, args, {
      shell: false, windowsHide: true, stdio: ["ignore", "pipe", "pipe"]
    });
    const chunks = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    let error = null;
    const fail = reason => {
      if (error) return;
      error = new Error(reason);
      // For this isolated CI run DiskSpd has no child workload processes.
      // This is not the Windows Job Object tree cancellation gate.
      child.kill("SIGKILL");
    };
    const watchdog = setTimeout(() => fail("DiskSpd CI read test exceeded 25-second limit"), 25000);
    child.stdout.on("data", chunk => {
      stdoutBytes += chunk.length;
      if (stdoutBytes > MAX_XML_BYTES) { fail("DiskSpd XML output exceeded 1 MiB"); return; }
      chunks.push(chunk);
    });
    child.stderr.on("data", chunk => {
      stderrBytes += chunk.length;
      if (stderrBytes > MAX_STDERR_BYTES) fail("DiskSpd diagnostic output exceeded 16 KiB");
    });
    child.once("error", e => { clearTimeout(watchdog); reject(e); });
    child.once("close", code => {
      clearTimeout(watchdog);
      if (error) return reject(error);
      if (code !== 0) return reject(new Error("DiskSpd process failed (exit " + code + ")"));
      resolve(Buffer.concat(chunks).toString("utf8"));
    });
  });
}

async function main() {
  const binary = process.argv[2];
  const pinnedEngineVersion = process.argv[3];
  if (process.platform !== "win32" || process.env.GITHUB_ACTIONS !== "true" ||
      !process.env.RUNNER_TEMP || !binary ||
      !path.win32.isAbsolute(binary) ||
      path.win32.basename(binary).toLowerCase() !== "diskspd.exe" ||
      !/^\d+\.\d+\.\d+$/.test(pinnedEngineVersion || "")) {
    throw new Error("CI-only DiskSpd runner refuses this environment or binary");
  }
  const trustedRoot = await fs.realpath(process.env.RUNNER_TEMP);
  const lease = await OwnedScratchLease.createInsideTrustedTestRoot(trustedRoot, 64 * 1024 * 1024);
  try {
    const plan = buildDiskSpdFileArgs(lease.filePath, {
      profileId: "seq1m-q1t1", direction: "read",
      warmupSeconds: 0, measurementSeconds: 2, repetitions: 1
    });
    if (plan.expectedWrite || plan.args.some(arg => /^-w(?!0$)/.test(arg))) {
      throw new Error("CI read-only policy violation");
    }
    await lease.verify();
    const xml = await runDiskSpd(binary, plan.args);
    await lease.verify();
    const parsed = parseDiskSpdXml(xml, {
      profileId: plan.profileId, direction: "read",
      ownedTargetPath: lease.filePath, pinnedEngineVersion,
      measurementSeconds: plan.measurementSeconds
    });
    if (parsed.measurement.iops <= 0 || parsed.measurement.direction !== "read") {
      throw new Error("DiskSpd XML contained no valid read measurement");
    }
    // Never publish the virtual runner's throughput as an SSD benchmark.
    console.log("CI safety check passed: exclusive scratch lease + fixed read-only DiskSpd argv + real XML");
  } finally {
    await lease.cleanup();
  }
}
main().catch(error => {
  console.error("P2.5B CI safety check failed:", error.message);
  process.exitCode = 1;
});
