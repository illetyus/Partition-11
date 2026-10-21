"use strict";
// CI-only integration check. Not included in the packaged app.
const fs = require("node:fs");
const path = require("node:path");
const { parseDiskSpdXml } = require("../dist/main/main/benchmarkXml.js");

const [xmlPath, expectedScratchFile] = process.argv.slice(2);
if (!xmlPath || !expectedScratchFile) {
  console.error("usage: node tests/check-real-diskspd-xml.cjs <result.xml> <owned-scratch-file>");
  process.exit(2);
}
try {
  const xml = fs.readFileSync(path.resolve(xmlPath), "utf8");
  const parsed = parseDiskSpdXml(xml, {
    profileId: "seq1m-q1t1",
    direction: "read",
    ownedTargetPath: expectedScratchFile,
    pinnedEngineVersion: "2.3.0",
    measurementSeconds: 15
  });
  if (parsed.measurement.direction !== "read" || parsed.measurement.iops <= 0) {
    throw new Error("Invalid read-only smoke result");
  }
  // CI runner performance is NOT a representative consumer-device score.
  console.log("DISKSPD XML VERIFIED: schema, scratch file, preset, latency and counters");
} catch (error) {
  console.error("DiskSpd XML integration failed:", error.message);
  process.exit(1);
}
