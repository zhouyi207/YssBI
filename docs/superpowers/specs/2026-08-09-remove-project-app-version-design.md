# Remove Project Application Version Metadata Design

**Date:** 2026-08-09
**Status:** Implemented and verified
**Scope:** Remove application release-version metadata from project persistence, IPC/domain models, snapshots, and ordinary tests while preserving actual format/protocol/revision versions.

## 1. Problem

`appVersion` / `app_version` is currently required across project metadata, project manifests, project-index IPC, frontend domain models, snapshots, and many unrelated test fixtures. Most consumers only pass the value through and never make a compatibility or UI decision from it. Tests therefore copy release-looking literals such as `0.2.7`, while frontend production code independently writes the incorrect hard-coded value `1.0.0`.

Project-file compatibility is already owned by `ProjectManifest.schema_version`. Keeping an unused application release version creates multiple authorities without adding compatibility safety.

## 2. Decision

Delete project application-version metadata end to end:

- Rust `ProjectMetadata.app_version`;
- Rust `ProjectManifest.app_version`;
- Rust `ProjectIndex.app_version`;
- project load/save/index mapping for that field;
- TypeScript `ProjectMetadata.appVersion`;
- TypeScript `ProjectIndexRow.appVersion` and its strict parser key;
- frontend snapshot/load-plan metadata propagation;
- every unrelated test fixture field and assertion.

No deprecated alias, optional compatibility field, default, or migration shim will be added. Existing persisted `app_version` keys may be ignored by normal serde unknown-field behavior, but no code will read or preserve them.

`appVersion` and `app_version` are globally reserved names and are forbidden in all project TypeScript/Rust source and ordinary tests, regardless of whether a type represents YssBI state or an external runtime. Legitimate application/runtime releases use `version`, `runtimeVersion`, `runtime_version`, or uppercase `APP_VERSION`; schema/revision/semantics/wire/exchange concepts retain their existing precise names. The only exceptions are architecture-audit implementation/negative fixture literals and the strict parser's dynamically constructed legacy-key rejection.

## 3. Preserved Version Concepts

The change does not remove versions that carry real protocol or authority semantics:

- `ProjectManifest.schema_version`;
- worksheet/database schema versions;
- graph/resource/publication revisions;
- execution-semantics and wire protocol versions;
- exchange-manifest versions;
- external runtime version parsing such as Julia version output;
- package release declarations in `package.json` and `src-tauri/Cargo.toml`.

Tests for these contracts continue to use explicit version values where version behavior is the subject of the test.

## 4. Data Flow After Removal

### Rust persistence

`ProjectManifest` contains only format compatibility and project metadata that is consumed:

- `schema_version`;
- `project_name`;
- `export_time`.

Project creation initializes project name and export time. Loading and saving no longer copy application release data. `ProjectIndex` no longer exposes application version over IPC.

### Frontend projection

`ProjectMetadata` contains only `exportTime`. Project-index parsing requires the new exact key set without `appVersion`. `buildAuthoritativeProjectLoadPlan` and `exportSnapshot` propagate/generate only export time. Rust remains authoritative for persisted project state.

## 5. Strictness and Error Handling

- The frontend project-index parser rejects the removed `appVersion` field as an unknown key.
- Missing `appVersion` is valid because it is no longer part of the wire contract.
- Rust serialization no longer emits `app_version`.
- Project format compatibility remains checked only through `schema_version`.
- No test dynamically imports `package.json` merely to populate project fixtures.

## 6. Testing

RED-GREEN coverage will include:

1. Rust manifest serialization omits `app_version` and loading works without it.
2. Rust project index serialization omits `appVersion`.
3. TypeScript strict project-index parsing accepts the new shape and rejects legacy `appVersion`.
4. Frontend project load and snapshot tests compile and pass without application-version fixture data.
5. Global exact-name semantic audits ensure production and ordinary test sources do not expose `appVersion` / `app_version`. External-runtime contracts use `runtimeVersion` / `runtime_version` or `version`; package manifests, schema/revision/protocol/semantics/exchange names, uppercase `APP_VERSION`, audit negative fixtures, and the strict parser's dynamic legacy key remain valid.
6. Focused project I/O/service/store suites, frontend typecheck/full tests, Rust project tests/checks, and `git diff --check` pass.

Fix Round 5 is historical evidence. At its breaker, four findings were explicitly parked for final review: aliased/const-flow TypeScript standard APIs, Rust attribute-macro tokens, order-dependent serde parsing, and the redundant snapshot dynamic-key assertion. The final reviewer expanded the complete closure list to seven findings by adding exact-own-key parser/effect ordering, required project-index arrays with fallback removal, and active documentation/chronology cleanup.

The Final review fix wave is the current verification evidence:

- independent RED: TypeScript audit `9 failed / 36 passed`, Rust application-version audit `2 failed / 23 passed`, and strict parser `1 failed / 10 passed`; the snapshot suite stayed green at `23 passed` after removal of only the redundant assertion;
- focused GREEN: TypeScript architecture audit `45 passed`, Rust application-version audit `25 passed`, full Rust `source_audit` `71 passed`, strict parser `11 passed`, and snapshot `23 passed`;
- frontend: `251` test files and `1,617` tests passed;
- YssBI Rust: `1,367` library tests passed and `35` integration tests passed (`1,402` total);
- `yss-sci`: `43` tests passed and `1` test was ignored;
- `pnpm typecheck`, full `pnpm test`, `pnpm rust:fmt:check`, `pnpm rust:check`, serial full Rust library tests, canonical `pnpm verify`, and `git diff --check` exited 0;
- canonical `pnpm verify` exited `0`, the staged index remained empty, and the dirty tree was preserved.

The direct symbol inventory contains `44` `appVersion` matches only in the excluded TypeScript audit implementation/negative fixtures and `26` `app_version` matches only in the excluded Rust audit implementation/negative fixtures. Ordinary production and test sources contain none. The strict parser's dynamic legacy-key rejection is the sole ordinary exception; the snapshot duplicate was removed. All seven final findings are resolved with no open concern.

## 7. Non-goals

- Synchronizing `package.json` and Cargo package versions.
- Changing project `schema_version` or adding a migration framework.
- Removing schema, revision, protocol, semantics, exchange, or external-runtime versions.
- Refactoring unrelated project persistence or lifecycle code.
