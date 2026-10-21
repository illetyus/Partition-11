# Third-party license inventory and review gates

Partition 11-owned source is MIT. Dependency licenses do NOT automatically
become the project's own license. This is an inventory of *candidates*,
not a statement that they are all included in distributed builds.

| Upstream | License | Status / effect |
|---|---|---|
| Electron — https://github.com/electron/electron | MIT (check bundled subcomponents) | Packaged Electron runtime; retain upstream and Chromium/component notices |
| Fluent UI — https://github.com/microsoft/fluentui | MIT (fonts/assets may carry other terms) | Build-time dependency bundled into the renderer; retain notices and review any shipped fonts/assets |
| Rust fs-ext4 — https://github.com/antimatter-studios/rust-fs-ext4 | MIT | Exact v0.8.0 dependency of `partition11-ext-reader`, statically linked into the Windows agent for read-only ext4 image inspection; no mutation APIs exposed |
| Rust fs-core — https://github.com/antimatter-studios/rust-fs-core | MIT | Transitive dependency of the agent-linked `rust-fs-ext4` v0.8.0 (locked v0.3.15) |
| e2fsprogs — https://github.com/tytso/e2fsprogs | GPLv2 utilities, GNU Library GPL v2 libext2fs/libe2p, BSD-style libuuid, MIT-style libet/libss | External golden reference; not vendored |
| lwext4 — https://github.com/gkostka/lwext4 | BSD-3-Clause plus GPLv2 source files; complete build GPLv2 | Reference only, not vendored |
| WinFsp — https://github.com/winfsp/winfsp | GPLv3 with FLOSS exception / commercial option | Deferred; exception conditions apply |
| libfsext — https://github.com/libyal/libfsext | LGPL-3.0-or-later | Optional read-only candidate |
| Microsoft DiskSpd — https://github.com/microsoft/diskspd | MIT | v2.3 AMD64 is configured for the next unsigned Windows x64 preview with its upstream notice and CycloneDX entry; live benchmark execution is still gated |
| CrystalDiskMark — https://github.com/hiyohiyo/CrystalDiskMark | MIT headers verified in DiskBench.cpp; whole-asset review pending | UX/profile reference only, not bundled |
| fio — https://github.com/axboe/fio | GPL-2.0 COPYING verified | Development reference only, not bundled |
| fast-xml-parser — https://github.com/NaturalIntelligence/fast-xml-parser | MIT | Production dependency for strict DiskSpd XML result parsing; benchmark execution remains disabled pending P2.5 gates |
| bitflags | MIT OR Apache-2.0 | Transitive dependency of `rust-fs-ext4` v0.8.0 (locked v2.13.2) |
| caseless | MIT | Transitive dependency of `rust-fs-ext4` v0.8.0 (locked v0.2.2) |
| crc32c | Apache-2.0 OR MIT | Transitive dependency of `rust-fs-ext4` v0.8.0 (locked v0.6.8) |
| tinyvec | Zlib OR Apache-2.0 OR MIT | Transitive dependency of `rust-fs-ext4` v0.8.0 (locked v1.13.3) |
| unicode-normalization | (MIT OR Apache-2.0) AND Unicode-3.0 | Transitive dependency of `rust-fs-ext4` v0.8.0 (locked v0.1.26) |
| rustc_version | MIT OR Apache-2.0 | Build dependency of `crc32c` (locked v0.4.1) |
| semver | MIT OR Apache-2.0 | Build dependency of `rustc_version` (locked v1.0.28) |

## Gate

- Never copy e2fsprogs GPL-2.0-only or Rufus GPLv3 source into the MIT
  application without an explicit licensing/architecture review.
- The WinFsp FLOSS exception requires attribution and excludes proprietary
  mixes under the exception. Review the exact bundled distribution.
- External executable/IPC boundaries are NOT an automatic copyright-law
  exemption. Review combination and distribution before release.
- Record a pinned upstream revision, SPDX identifier, modifications,
  copyright notices, bundled licenses, and all transitive packages.
- Run dependency and SBOM tooling in release pipeline; CI in P0 only
  proves compilation and policy tests, not legal clearance.

## Dependency lockfiles and audits

- The checked-in `package-lock.json` records the resolved npm dependency graph,
  but does not itself prove that every transitive package has an approved license.
- `Cargo.lock` pins the Rust workspace graph. The Windows `partition11-agent` links `partition11-core` and `partition11-ext-reader`; the latter pins `rust-fs-ext4` v0.8.0 and the transitive packages listed above for read-only ext4 image inspection. The agent CycloneDX is regenerated for `x86_64-pc-windows-msvc` and shipped beside the agent in the Windows preview; it does not cover Electron/Chromium or Windows system components.
- Windows CI uses `npm ci` and Cargo `--locked` to reject dependency
  resolution drift; CycloneDX SBOM output is archived per CI run.
- Before a public binary release, inspect Electron/Chromium bundled third-party
  notices, native redistributables, font/icon licenses, and dependency changes.
  An automated SBOM is evidence for review, not a license grant or legal opinion.

## Benchmark license/release checks

The current preview pin is recorded in third_party/diskspd/manifest.json and
third_party/diskspd/diskspd.cdx.json:
Microsoft DiskSpd v2.3, the official DiskSpd.ZIP release asset, SHA-256
18609981e8b04b4f645f1fd182ad0541615fe71722de36a9a933c54f747218a3, and
the amd64 executable. The matching upstream MIT notice is preserved in
third_party/diskspd/LICENSE. The package configuration now includes the pinned
AMD64 executable, notice, manifest, and component-level CycloneDX SBOM in the
next unsigned Windows x64 preview. No new ZIP has been built from this change.
The current Windows workflows validate those resources and the executable
digest; the updated workflow has not yet run for this local branch.

Before a public release, review the exact bundled distribution, all notices,
and the complete application SBOM. Do not dynamically download executables at
application runtime. CrystalDiskMark source/assets are not copied. fio remains
a separate, non-distributed test comparator pending any explicit legal review.



## Current Windows x64 agent SBOM verification — 2026-10-20

The exact CI generator command produced an 11-component CycloneDX 1.5 SBOM for
`partition11-agent` targeting `x86_64-pc-windows-msvc`. It includes
`partition11-core`, `partition11-ext-reader` 0.1.0, and `rust-fs-ext4` 0.8.0,
with MIT license expressions. The dependency graph binds the agent to the image
reader and the image reader to the pinned ext4 engine. This is package inventory
evidence, not legal clearance for Electron/Chromium subcomponents, Windows SDK
redistributables, or fonts/assets.
