import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { parseDiskSpdXml, type TrustedDiskSpdRun } from "./benchmarkXml";

const readContext: TrustedDiskSpdRun = {
  profileId: "seq1m-q8t1", direction: "read", ownedTargetPath: "P11Owned_A.bin",
  pinnedEngineVersion: "2.3"
};
const writeContext: TrustedDiskSpdRun = {
  profileId: "rnd4k-q32t1", direction: "write", ownedTargetPath: "P11Owned_B.bin",
  pinnedEngineVersion: "2.3"
};
const readXml = readFileSync(path.resolve("tests/benchmark-fixtures/seq-read.xml"), "utf8");
const writeXml = readFileSync(path.resolve("tests/benchmark-fixtures/rnd-write.xml"), "utf8");

test("schema-faithful synthetic read fixture gives decimal MB/s, IOPS and latency", () => {
  const r = parseDiskSpdXml(readXml, readContext);
  assert.equal(r.durationSeconds, 15);
  assert.equal(r.measurement.throughputMBps, 104.8576);
  assert.equal(r.measurement.iops, 100);
  assert.equal(r.measurement.latencyAvgMs, 0.4);
  assert.equal(r.measurement.latencyP95Ms, 0.72);
  assert.equal(r.measurement.latencyP99Ms, 1.12);
});
test("schema-faithful synthetic write fixture parses 4K Q32 and keeps direction", () => {
  const r = parseDiskSpdXml(writeXml, writeContext);
  assert.equal(r.measurement.direction, "write");
  assert.ok(Math.abs(r.measurement.throughputMBps - 27.3066666667) < 1e-9);
  assert.ok(Math.abs(r.measurement.iops - 100000 / 15) < 1e-9);
  assert.equal(r.measurement.latencyP95Ms, 1.9);
});
test("rejects invalid XML, DTD, nested entity, comments and excessive output", () => {
  for (const xml of [
    "", "nonsense", "<Results/>", "<Results><TimeSpan></Results>",
    '<!DOCTYPE Results [<!ENTITY xxe SYSTEM "file:///C:/secret">]>' + readXml,
    '<Results>&xxe;</Results>',
    readXml.replace("<Results>", "<Results><!--inject-->"),
    " ".repeat(1024 * 1024 + 2) + readXml
  ]) {
    assert.throws(() => parseDiskSpdXml(xml, readContext));
  }
});
test("rejects forged version, path, workload, concurrency and tampered counters", () => {
  const cases = [
    [readXml.replace("<Version>2.3</Version>", "<Version>9.9</Version>"), readContext],
    [readXml.replace("P11Owned_A.bin", "P11Owned_other.bin"), readContext],
    [readXml.replace("<RequestCount>8</RequestCount>", "<RequestCount>64</RequestCount>"), readContext],
    [readXml.replace("<WriteRatio>0</WriteRatio>", "<WriteRatio>100</WriteRatio>"), readContext],
    [readXml.replace("<ReadBytes>1572864000</ReadBytes>", "<ReadBytes>999</ReadBytes>"), readContext],
    [readXml.replace("<ThreadCount>1</ThreadCount>", "<ThreadCount>2</ThreadCount>"), readContext],
    [readXml.replace("<TestTimeSeconds>15.00</TestTimeSeconds>", "<TestTimeSeconds>0</TestTimeSeconds>"), readContext],
    [readXml.replace("<ReadCount>1500</ReadCount>", "<ReadCount>9007199254740999</ReadCount>"), readContext],
    [readXml.replace("</Thread>", "</Thread><Thread><Target><Path>other</Path></Target></Thread>"), readContext],
    [writeXml, readContext]
  ] as const;
  for (const [xml, context] of cases) assert.throws(() => parseDiskSpdXml(xml, context));
});
test("refuses interrupted, mixed and inconsistent directions and invalid latency", () => {
  assert.throws(() => parseDiskSpdXml(readXml.replace("<TimeSpan><TestTimeSeconds>", "<TimeSpan><Error>interrupt</Error><TestTimeSeconds>"), readContext));
  assert.throws(() => parseDiskSpdXml(writeXml.replace("<ReadCount>0</ReadCount>", "<ReadCount>1</ReadCount>"), writeContext));
  assert.throws(() => parseDiskSpdXml(readXml.replace("<Percentile>99</Percentile>", "<Percentile>95</Percentile>"), readContext));
  assert.throws(() => parseDiskSpdXml(readXml.replace("<ReadMilliseconds>1.120</ReadMilliseconds>", "<ReadMilliseconds>0.010</ReadMilliseconds>"), readContext));
});
