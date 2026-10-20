"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const repoRoot = path.resolve(__dirname, "..");
const manifest = JSON.parse(fs.readFileSync(path.join(repoRoot, "third_party/diskspd/manifest.json"), "utf8"));
const { BENCHMARK_ENGINE_VERSION } = require(path.join(repoRoot, "dist/main/shared/benchmark.js"));

test("Bundled DiskSpd has pinned upstream provenance and an MIT notice", () => {
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.upstreamRepository, "https://github.com/microsoft/diskspd");
  assert.equal(manifest.releaseTag, "v2.3");
  assert.match(manifest.releaseCommitShort, /^[0-9a-f]{7}$/);
  assert.equal(manifest.assetName, "DiskSpd.ZIP");
  assert.equal(manifest.assetUrl, "https://github.com/microsoft/diskspd/releases/download/v2.3/DiskSpd.ZIP");
  assert.equal(manifest.archiveSha256, "18609981e8b04b4f645f1fd182ad0541615fe71722de36a9a933c54f747218a3");
  assert.equal(manifest.binaryArchitecture, "amd64");
  assert.equal(manifest.xmlResultVersion, "2.3.0");
  assert.equal(manifest.xmlResultVersion, BENCHMARK_ENGINE_VERSION);
  assert.equal(manifest.license, "MIT");
  assert.equal(manifest.distributionStatus, "bundled-in-unsigned-preview");

  const licensePath = path.join(repoRoot, manifest.licenseFile);
  const license = fs.readFileSync(licensePath, "utf8");
  assert.match(license, /The MIT License \(MIT\)/);
  assert.match(license, /Copyright \(c\) 2014 Microsoft/);
});

test("bundled AMD64 DiskSpd executable is hash-pinned to the upstream archive", () => {
  const crypto = require("node:crypto");
  const binaryPath = path.join(repoRoot, "third_party/diskspd/amd64/diskspd.exe");
  assert.equal(manifest.executableSha256, "dd4e57e1e8ccaf5d6437938f8aab7f17e9a1e6d8fba8a093006b7cadf16faea2");
  assert.equal(fs.existsSync(binaryPath), true, "pinned executable must be present for packaging");
  const digest = crypto.createHash("sha256").update(fs.readFileSync(binaryPath)).digest("hex");
  assert.equal(digest, manifest.executableSha256);
});
test("DiskSpd CycloneDX records upstream version, architecture, MIT license and executable hash", () => {
  const bomPath = path.join(repoRoot, "third_party/diskspd/diskspd.cdx.json");
  const bom = JSON.parse(fs.readFileSync(bomPath, "utf8"));
  assert.equal(bom.bomFormat, "CycloneDX");
  assert.equal(bom.specVersion, "1.5");
  const engine = bom.components.find(component => component.name === "Microsoft DiskSpd AMD64");
  assert.ok(engine);
  assert.equal(engine.version, "2.3");
  assert.equal(engine.licenses[0].license.id, "MIT");
  assert.equal(engine.hashes.find(hash => hash.alg === "SHA-256").content, manifest.executableSha256);
  assert.ok(engine.externalReferences.some(reference => reference.url === manifest.assetUrl));
  assert.ok(engine.properties.some(property => property.name === "binaryArchitecture" && property.value === "amd64"));
  assert.ok(engine.properties.some(property => property.name === "archiveSha256" && property.value === manifest.archiveSha256));
});
test("Windows preview explicitly packages the pinned engine and notices", () => {
  const packageJson = JSON.parse(fs.readFileSync(path.join(repoRoot, "package.json"), "utf8"));
  const resources = packageJson.build.extraResources;
  assert.ok(resources.some(resource => resource.from === "third_party/diskspd/amd64/diskspd.exe" && resource.to === "diskspd/amd64/diskspd.exe"));
  assert.ok(resources.some(resource => resource.from === "third_party/diskspd/LICENSE" && resource.to === "diskspd/LICENSE"));
  assert.ok(resources.some(resource => resource.from === "third_party/diskspd/manifest.json" && resource.to === "diskspd/manifest.json"));
  assert.ok(resources.some(resource => resource.from === "third_party/diskspd/diskspd.cdx.json" && resource.to === "diskspd/diskspd.cdx.json"));
});
test("package contains only the intended Windows agent and DiskSpd resources", () => {
  const packageJson = JSON.parse(fs.readFileSync(path.join(repoRoot, "package.json"), "utf8"));
  assert.deepEqual(packageJson.build.files, ["dist/**/*", "!dist/main/**/*.test.js", "package.json", "LICENSE"]);
  assert.deepEqual(packageJson.build.extraResources.map(resource => resource.to).sort(), [
    "agent/partition11-agent.exe",
    "diskspd/LICENSE",
    "diskspd/amd64/diskspd.exe",
    "diskspd/manifest.json",
    "diskspd/diskspd.cdx.json"
  ].sort());
  assert.deepEqual(
    fs.readdirSync(path.join(repoRoot, "third_party/diskspd")).sort(),
    ["LICENSE", "amd64", "diskspd.cdx.json", "manifest.json"]
  );
});
