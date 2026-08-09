# Remove Project Application Version Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `appVersion` / `app_version` from project persistence, project-index IPC, frontend project models, snapshots, and unrelated tests while preserving real schema/protocol/revision versions.

**Architecture:** Rust project persistence remains authoritative and uses `ProjectManifest.schema_version` as the sole project-format compatibility version. The frontend consumes a strict project-index wire without application release metadata and keeps only export time in transient project snapshots. Global TypeScript and Rust semantic audits reserve the exact names `appVersion` / `app_version` while allowing precisely named package, runtime, schema, protocol, revision, and exchange-manifest versions.

**Tech Stack:** Rust 2024, serde, syn, Tauri 2, TypeScript 5.8, Zustand, Vitest 4, pnpm 11.

## Global Constraints

- Work in the existing dirty `shadcn` workspace and preserve all unrelated Task 1–19/user changes.
- Do not create commits, staging, branches, worktrees, tags, merges, pushes, resets, or reverts.
- Delete the field directly; do not add a deprecated alias, optional compatibility field, default, or migration shim.
- Keep `ProjectManifest.schema_version` and all schema, protocol, revision, semantics, exchange-manifest, and external-runtime version contracts.
- Reserve `appVersion` / `app_version` globally in TypeScript/Rust source and ordinary tests. External-runtime contracts use `runtimeVersion` / `runtime_version` or `version`; uppercase `APP_VERSION` remains valid.
- Exceptions are limited to architecture-audit implementation/negative fixture literals and the strict parser's dynamic legacy-key rejection.
- Rust remains authoritative for persisted project data; frontend views/stores do not invent application release metadata.
- Add focused RED before production changes and run Cargo only through root `pnpm` scripts.

---

### Task 1: Remove application version from Rust project persistence and index wire

**Files:**
- Modify: `src-tauri/src/project/project_metadata.rs`
- Modify: `src-tauri/src/project/project_io.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_io.rs` test module if the focused serialization helpers are colocated there

**Interfaces:**
- Produces `ProjectMetadata { project_name: String, export_time: String }` with no `app_version`.
- Produces `ProjectManifest { schema_version: u32, project_name: String, export_time: String }` with no `app_version`.
- Produces `ProjectIndex` JSON without `appVersion`.
- Preserves `SCHEMA_VERSION` as project-format compatibility authority.

- [x] **Step 1: Add manifest serialization RED**

Add a focused Rust test that serializes a project manifest and checks its exact keys:

```rust
let value: serde_json::Value = serde_json::from_slice(
    &serialize_project_manifest(&ProjectData::new()).expect("serialize manifest"),
).expect("manifest JSON");
let object = value.as_object().expect("manifest object");
assert_eq!(
    object.keys().map(String::as_str).collect::<std::collections::BTreeSet<_>>(),
    std::collections::BTreeSet::from(["schemaVersion", "projectName", "exportTime"]),
);
assert!(!object.contains_key("appVersion"));
```

- [x] **Step 2: Run the manifest RED**

Run:

```text
pnpm rust:test --lib project_manifest_omits_application_version -- --test-threads=1
```

Expected: FAIL because `appVersion` is still serialized.

- [x] **Step 3: Add project-index wire RED**

Construct/serialize the real `ProjectIndex` returned by project reads and assert the exact top-level key inventory excludes `appVersion` while retaining `projectInstanceId`, `publicationRevision`, `history`, `projectName`, `exportTime`, `graphs`, `worksheets`, `variables`, and `databases`.

- [x] **Step 4: Run the index RED**

Run:

```text
pnpm rust:test --lib project_index_omits_application_version -- --test-threads=1
```

Expected: FAIL because `ProjectIndex.app_version` still emits `appVersion`.

- [x] **Step 5: Remove Rust application-version fields and mappings**

Apply these exact structural changes:

```rust
pub struct ProjectMetadata {
    #[serde(default)]
    pub project_name: String,
    pub export_time: String,
}

pub struct ProjectManifest {
    pub schema_version: u32,
    pub project_name: String,
    pub export_time: String,
}

pub struct ProjectIndex {
    pub project_instance_id: String,
    pub publication_revision: u64,
    pub history: HistoryStatusDto,
    pub project_name: String,
    pub export_time: String,
    // existing resource arrays unchanged
}
```

Remove every load/save/index assignment of `app_version`. Keep `schema_version` validation and all other manifest fields unchanged. `ProjectMetadata::default()` initializes only project name and export time; it no longer reads `CARGO_PKG_VERSION`.

- [x] **Step 6: Run focused Rust GREEN**

Run:

```text
pnpm rust:test --lib project_manifest_omits_application_version -- --test-threads=1
pnpm rust:test --lib project_index_omits_application_version -- --test-threads=1
pnpm rust:test --lib project_io -- --test-threads=1
pnpm rust:check
```

Expected: all pass; Rust project serialization contains no `app_version`/`appVersion` field.

---

### Task 2: Remove application version from frontend project wire, domain, snapshots, and fixtures

**Files:**
- Modify: `src/services/project/projectService.ts`
- Modify: `src/services/project/projectService.test.ts`
- Modify: `src/shared/types/domain/project.ts`
- Modify: `src/features/core/dataStore/authoritativeProjectLoadPlan.ts`
- Modify: `src/features/core/dataStore/projectIOStore.ts`
- Modify: `src/features/core/dataStore/projectIOStore.test.ts`
- Modify: frontend tests returned by the focused `appVersion` inventory under `src/features/application/editorMutation/`, `src/features/application/project/`, `src/features/core/sync/handlers/`, and project lifecycle tests

**Interfaces:**
- Produces `ProjectIndexRow` without `appVersion`.
- Produces frontend `ProjectMetadata { exportTime: string }`.
- `parseProjectIndexRow(unknown)` accepts only the exact Rust top-level index keys and rejects legacy `appVersion`.
- `exportSnapshot()` generates only `metadata.exportTime`; it no longer writes hard-coded `1.0.0`.

- [x] **Step 1: Add strict project-index parser RED**

In `projectService.test.ts`, update a valid Rust-shaped index to omit `appVersion`, then assert:

```ts
await expect(ProjectService.getProjectIndex(projectInstanceId)).resolves.toMatchObject({
  projectInstanceId,
  projectName: 'Projection contract',
});
expect(result).not.toHaveProperty('appVersion');
```

Add a legacy-field rejection case:

```ts
vi.mocked(invoke).mockResolvedValue({ ...validIndex, appVersion: '0.2.7' });
await expect(ProjectService.getProjectIndex(projectInstanceId))
  .rejects.toThrow('Invalid project index response');
```

- [x] **Step 2: Run parser RED**

Run:

```text
pnpm test src/services/project/projectService.test.ts
```

Expected: FAIL because the TypeScript interface/parser still accepts and forwards `appVersion`.

- [x] **Step 3: Make the project-index parser exact**

Remove `appVersion` from `ProjectIndexRow`. Require this exact top-level key set:

```ts
const PROJECT_INDEX_KEYS = [
  'projectInstanceId',
  'publicationRevision',
  'history',
  'projectName',
  'exportTime',
  'graphs',
  'worksheets',
  'variables',
  'databases',
] as const;
```

`parseProjectIndexRow` must call `hasExactKeys`, validate primitive fields and arrays, parse every graph with `parseProjectGraphIndexRow`, and validate database entries with `isProjectDatabaseIndexRow`. It must reject unknown `appVersion` before returning any value.

- [x] **Step 4: Add snapshot RED**

Update `projectIOStore.test.ts` so the expected snapshot metadata is:

```ts
expect(snapshot.metadata).toEqual({
  exportTime: expect.any(String),
});
expect(snapshot.metadata).not.toHaveProperty('appVersion');
```

- [x] **Step 5: Run snapshot RED**

Run:

```text
pnpm test src/features/core/dataStore/projectIOStore.test.ts
```

Expected: FAIL because production `exportSnapshot()` still writes `appVersion: '1.0.0'`.

- [x] **Step 6: Remove frontend application-version propagation**

Change the domain type to:

```ts
export interface ProjectMetadata {
  exportTime: string;
}
```

Remove `appVersion` from `buildAuthoritativeProjectLoadPlan`, project lifecycle clear-state metadata, `exportSnapshot`, and all ProjectIndex/ProjectData fixtures. Do not replace literals with a shared test version constant; the field must disappear.

- [x] **Step 7: Run focused frontend GREEN**

Run:

```text
pnpm test src/services/project/projectService.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/application/editorMutation src/features/application/project src/features/core/sync/handlers
pnpm typecheck
```

Expected: all pass with no `appVersion` fixture requirement and no hard-coded production application version.

---

### Task 3: Add semantic boundary audit and complete verification

**Files:**
- Create: `src/services/nodeSystem/projectApplicationVersionArchitectureContract.test.ts`
- Modify: `src-tauri/src/node_system/testing/source_audit.rs`
- Modify: `docs/superpowers/specs/2026-08-09-remove-project-app-version-design.md` status/evidence
- Modify: `docs/superpowers/plans/2026-08-09-remove-project-app-version.md` checkboxes/evidence

**Interfaces:**
- Rust `syn` audit globally rejects exact `app_version` identifiers in fields, items, locals, members, paths, and imports, plus serde `appVersion` renames, under project `src` and `tests` roots.
- TypeScript Program/TypeChecker/AST audit globally rejects declarations, static properties/accesses, checker types, aliases, re-exports, and call/construction results exposing exact `appVersion`.
- Audits allow package manifest versions, `schemaVersion`, revisions, semantics/wire versions, exchange manifest versions, `runtimeVersion` / `runtime_version`, `version`, and uppercase `APP_VERSION`.

- [x] **Step 1: Add architecture-audit RED fixtures**

Add semantic fixtures that must fail:

```rust
struct ProjectManifest { app_version: String }
```

```ts
interface ProjectIndexRow { appVersion: string }
const snapshot = { metadata: { exportTime: '', appVersion: '1.0.0' } };
```

Add decoys that must pass:

```rust
struct ExchangeManifest { version: u32 }
struct Worksheet { schema_version: u32 }
```

```ts
const schemaVersion = 3;
const resourceRevision = 7;
```

- [x] **Step 2: Run audit RED**

Run:

```text
pnpm rust:test --lib project_application_version_audit -- --test-threads=1
pnpm test src/services/nodeSystem/projectApplicationVersionArchitectureContract.test.ts
```

Expected: FAIL because no project application-version boundary audit exists.

- [x] **Step 3: Implement precise audits**

Use `syn` to inventory exact `app_version` AST identifiers and serde `appVersion` renames globally in project Rust `src`/`tests`, excluding only `source_audit.rs`; wildcard exposure is covered by scanning every forbidden declaration. Use TypeScript `Program`/`TypeChecker`/AST to reject exact static property names and checker-exposed types globally, excluding only the architecture audit file. Do not raw-scan comments or inert strings, and do not execute dynamic method calls such as the strict parser fixture's `join()`.

- [x] **Step 4: Run focused audit GREEN**

Run:

```text
pnpm rust:test --lib project_application_version_audit -- --test-threads=1
pnpm rust:test --lib source_audit -- --test-threads=1
pnpm test src/services/nodeSystem/projectApplicationVersionArchitectureContract.test.ts
```

Expected: all audit fixtures and production inventories pass.

- [x] **Step 5: Verify no project application-version symbols remain**

Run targeted repository searches for `appVersion` and `app_version`. Expected remaining matches: none in project production/test code. Package/Cargo version declarations and legitimate non-project version tests remain unchanged.

- [x] **Step 6: Run complete verification**

Run in order:

```text
pnpm typecheck
pnpm test
pnpm rust:fmt:check
pnpm rust:check
pnpm rust:test --lib -- --test-threads=1
pnpm verify
git --no-pager diff --check
git --no-pager diff --cached --name-only
git --no-optional-locks status --short
```

Expected: zero failures, canonical `pnpm verify` exit 0, no staged files, and only intended dirty/untracked changes.

- [x] **Step 7: Update design and plan evidence**

Mark the design implemented and record exact fresh test counts only after Step 6 succeeds. Check completed plan steps without changing unrelated Node Architecture evidence or TODO items.

## Verification evidence

Historical initial Task 3 evidence (superseded by Fix Round 5):

- RED: `pnpm rust:test --lib project_application_version_audit -- --test-threads=1` exited `101` with `2 failed / 2 passed`; `pnpm test src/services/nodeSystem/projectApplicationVersionArchitectureContract.test.ts` exited `1` with `2 failed / 2 passed`. Both failures were the expected missing-detector fixture failures.
- Focused GREEN: application-version Rust audit `4 passed`; Rust `source_audit` filter `50 passed`; TypeScript architecture audit `4 passed`.
- Symbol inventory: `6` direct `appVersion` matches, all in the excluded TypeScript audit implementation/negative fixture literals; `10` direct `app_version` matches, all in the excluded Rust audit implementation/negative fixture literals. Ordinary project production/test model sources contain zero direct matches, and the dynamic strict parser rejection remains unchanged.
- Complete sequence: `pnpm typecheck` exit `0`; `pnpm test` exit `0` with `251` files / `1,573` tests; `pnpm rust:fmt:check` exit `0`; `pnpm rust:check` exit `0`; `pnpm rust:test --lib -- --test-threads=1` exit `0` with `1,346` tests; canonical `pnpm verify` exit `0` with frontend `1,573`, YssBI Rust `1,381`, and `yss-sci` `43 passed / 1 ignored`; `git --no-pager diff --check` exit `0`; `git --no-pager diff --cached --name-only` exit `0` with empty output; `git --no-optional-locks status --short` exit `0` with only preserved dirty work and intended Task 1–3 files.
- One extra count-collection `pnpm rust:test` rerun encountered a transient Windows DuckDB file-handle collision in `test_edit_save_persists_to_duckdb`; the focused test then passed `1/1`, and a fresh canonical `pnpm verify` passed in full without code changes.

Historical Fix Round 4 evidence (superseded by Fix Round 5):

- Global-reservation RED: TypeScript `5 failed / 23 passed`; Rust `7 failed / 8 passed`. Failures were the independent external runtime, import/alias/new, item/local/path/serde/wildcard, and diagnostic fixtures that the provenance heuristics incorrectly allowed.
- Focused GREEN: TypeScript architecture audit `28 passed`; Rust application-version audit `15 passed`; full Rust `source_audit` `61 passed`; strict parser `8 passed`.
- Complete sequence: `pnpm typecheck`, `pnpm rust:fmt:check`, `pnpm rust:check`, `pnpm verify`, and `git diff --check` exited `0`; frontend `251` files / `1,597` tests passed; YssBI Rust `1,357` library plus `35` integration tests (`1,392` total) passed; `yss-sci` reported `43 passed / 1 ignored`.
- Inventory: `19` direct TypeScript `appVersion` matches and `13` direct Rust `app_version` matches, all confined to excluded audit implementation/negative fixtures. Ordinary source/tests contain zero. The parser exception remains dynamic.
- Diagnostics use file/symbol output for this audit rather than reporting a guessed line. The staged index remained empty.

Historical Fix Round 5 evidence (superseded by the Final review fix wave):

- Independent RED: TypeScript `7 failed / 29 passed`; Rust `5 failed / 16 passed`. The failures independently covered renamed destructuring, JSX, direct Object/Reflect property-key APIs, immediate `Object.fromEntries`, nested macro tokens, container/variant serde attributes, directional field rename, and alias handling.
- Focused GREEN: TypeScript architecture audit `36 passed`; Rust application-version audit `21 passed`; full Rust `source_audit` `67 passed`; strict parser `8 passed`.
- Complete sequence: frontend `251` files / `1,605` tests; YssBI Rust `1,398` total; `yss-sci` `43 passed / 1 ignored`; canonical verification and the empty-index check passed.
- At the breaker, four findings remained parked: aliased/const-flow standard APIs, Rust attribute-macro tokens, order-dependent serde parsing, and the redundant snapshot dynamic-key assertion. The final reviewer expanded the complete closure list to seven with parser own-key/effect ordering, required-array fallback cleanup, and active documentation chronology/mapping.

Final review fix wave current evidence:

- Independent RED: TypeScript audit `9 failed / 36 passed`; Rust application-version audit `2 failed / 23 passed`; strict parser `1 failed / 10 passed`. Snapshot remained green at `23 passed` after removing only the redundant assertion.
- Focused GREEN: TypeScript architecture audit `45 passed`; Rust application-version audit `25 passed`; full Rust `source_audit` `71 passed`; strict parser `11 passed`; snapshot `23 passed`.
- Exact full sequence passed in order: `pnpm typecheck`; full frontend `251` files / `1,617` tests; `pnpm rust:fmt:check`; `pnpm rust:check`; serial Rust library `1,367` tests; canonical `pnpm verify` exit `0` with YssBI Rust `1,402` total (`1,367` library + `35` integration) and `yss-sci` `43 passed / 1 ignored`; `git diff --check`; empty cached name-only output; preserved dirty-tree status.
- Inventory: `44` direct TypeScript `appVersion` matches and `26` direct Rust `app_version` matches, all confined to excluded audit implementation/negative fixtures. Ordinary source/tests contain zero; the strict parser's dynamic legacy-key rejection is the sole ordinary exception.
- All seven final findings are resolved. No Critical, Important, Minor, or open concern remains.
