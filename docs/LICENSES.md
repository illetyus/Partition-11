# Third-party license inventory and review gates

Partition 11-owned source is MIT. Dependency licenses do NOT automatically
become the project's own license. This is an inventory of *candidates*,
not a statement that they are all included in distributed builds.

| Upstream | License | Status / effect |
|---|---|---|
| Electron — https://github.com/electron/electron | MIT (check bundled subcomponents) | Planned npm dependency; retain notices |
| Fluent UI — https://github.com/microsoft/fluentui | MIT (fonts/assets may carry other terms) | Planned npm dependency; retain notices |
| Rust fs-ext4 — https://github.com/antimatter-studios/rust-fs-ext4 | MIT | Evaluation candidate; NOT yet linked |
| Rust fs-core — https://github.com/antimatter-studios/rust-fs-core | MIT | Transitive evaluation candidate |
| e2fsprogs — https://github.com/tytso/e2fsprogs | GPLv2 utilities, GNU Library GPL v2 libext2fs/libe2p, BSD-style libuuid, MIT-style libet/libss | External golden reference; not vendored |
| lwext4 — https://github.com/gkostka/lwext4 | BSD-3-Clause plus GPLv2 source files; complete build GPLv2 | Reference only, not vendored |
| WinFsp — https://github.com/winfsp/winfsp | GPLv3 with FLOSS exception / commercial option | Deferred; exception conditions apply |
| libfsext — https://github.com/libyal/libfsext | LGPL-3.0-or-later | Optional read-only candidate |

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
- `Cargo.lock` pins the Rust workspace graph; the current Rust agent only uses
  the workspace safety-policy crate and does **not** link an ext4 engine.
- Windows CI uses `npm ci` and Cargo `--locked` to reject dependency
  resolution drift; CycloneDX SBOM output is archived per CI run.
- Before a public binary release, inspect Electron/Chromium bundled third-party
  notices, native redistributables, font/icon licenses, and dependency changes.
  An automated SBOM is evidence for review, not a license grant or legal opinion.
