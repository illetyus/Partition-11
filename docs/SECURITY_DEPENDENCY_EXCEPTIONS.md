# Temporary dependency security exceptions

Reviewed: 2026-10-20

## GHSA-hp3w-g68c-fv3c — `sprintf-js`

- Severity: Moderate.
- Affected upstream versions: `sprintf-js` through 1.1.3; the GitHub Advisory
  Database currently lists no patched version.
- Current path: `electron-builder@26.15.3` → `app-builder-lib` →
  `@electron/get` → `global-agent` → `roarr` → `sprintf-js`.
- Scope: development and packaging dependencies. `electron-builder` and this
  transitive path are not part of the npm production dependency set or the
  packaged application's runtime dependency tree.
- Evidence: full `npm audit` reports eight moderate records for this one
  advisory path; `npm audit --omit=dev --audit-level=moderate` reports no
  production vulnerabilities. The advisory is
  [GHSA-hp3w-g68c-fv3c](https://github.com/advisories/GHSA-hp3w-g68c-fv3c).
- Decision: keep the known packaging tool version while upstream has no
  patched `sprintf-js` release. `npm audit fix --dry-run` proposes downgrading
  `electron-builder` from 26.15.3 to 26.5.0; that packaging-tool downgrade
  was not applied because it would roll back ten minor releases without a
  demonstrated compatibility benefit.
- CI treatment: fail on Moderate or higher for production dependencies; fail
  on High or Critical for the full graph. The full-graph report continues to
  surface this exception instead of suppressing it.
- Re-review: when upstream publishes a fix, before signing a release, or by
  2027-01-20, whichever comes first. Remove this exception as soon as the
  dependency path is fixed or the build-tool choice changes.

This is a narrow, temporary acceptance for a build-only dependency. It is not
a statement that the complete development dependency graph is vulnerability
free.
