# Revisioned Command and Database Recovery Design

## Status

Approved design for resolving the remaining Resource Catalog final-review blockers.

Execution constraints:

- Work directly on `shadcn`; do not create a worktree, branch, or commit.
- Preserve unrelated dirty work.
- Use focused frontend tests and serial focused Rust tests; do not run complete suites by default.
- Rust remains authoritative for project identity, resource declarations, revisions, and publication watermarks.
- `resource_revision` remains the sole public Catalog watermark.
- Resource paths remain opaque in frontend code.
- Do not add a database-specific event or receipt family.

## Problem

The Resource Catalog slice has three unresolved integration defects:

1. Revisioned frontend commands can read a domain revision before capturing the current project lifecycle. A project replacement between those reads can combine authority from different projects.
2. Project publication recovery receives only database identity, path, and revision. It cannot materialize a database created while the frontend missed its publication.
3. Resource delta validation strictly validates database nested objects but accepts unknown fields at the delta top level.

The database race also exposes the same authority-capture ordering in variable mutation and function-signature mutation. The correction must establish one reusable command-snapshot invariant rather than adding another database-only convention.

## 1. Revisioned command authority snapshot

Add a domain-agnostic synchronous helper under `features/application/projectCommandContext.ts`.

Its required sequence is:

1. Capture the current `ProjectCommandContext`, including project instance and command lifecycle.
2. Invoke a synchronous callback that reads the required domain authority.
3. Assert that the captured lifecycle is still current.
4. Return the context and authority as one immutable result.

The helper must not import database, variable, graph, or function stores. Domain callers supply the authority reader callback.

Conceptual interface:

```ts
interface RevisionedProjectCommandSnapshot<T> {
  context: ProjectCommandContext;
  authority: T;
}

function captureRevisionedProjectCommandSnapshot<T>(
  readAuthority: () => T,
): RevisionedProjectCommandSnapshot<T>;
```

The exact name may follow existing naming conventions, but the ordering and generic dependency boundary are mandatory.

### Migration scope

Migrate these known revisioned mutation paths:

- Database mutations: database revision.
- Variable mutations: variable revision and any scope facts required by the request.
- Function-signature mutations: function revision/signature authority used to construct the request.
- Existing graph revisioned mutations may adopt the helper where doing so is mechanical and behavior-preserving. Do not refactor unrelated graph workflows.

### Failure contract

If project replacement occurs while authority is read:

- fail before invoking Tauri;
- publish no mutation result;
- apply no optimistic state;
- do not pair an old authority value with a new project instance;
- preserve the existing stale-lifecycle error contract.

Missing authority remains a typed/localized application error and also produces zero IPC/publication effects.

## 2. Coherent database recovery projection

Extend the existing coherent Rust `ProjectIndex` database row. It must contain the canonical database declaration from the same authority generation as its path, revision, and publication watermark.

Required fields:

```text
id
resourcePath
revision
engine
schemaVersion
required
name
```

The declaration fields correspond to the canonical persisted `DatabaseDecl`. Runtime/editor enrichment such as columns, row counts, load errors, and transient loading state is not part of this authoritative recovery row.

### Rust projection

`ProjectState.project_data` remains authoritative. Project-index capture must project each database declaration together with:

- backend-issued opaque resource path;
- exact `database_authority_revisions` entry;
- the existing coherent project publication revision and authority generation.

The existing capture/revalidation protocol remains unchanged: no global lock may be held during filesystem I/O, and publication occurs only after project instance and authority generation revalidation.

Strict serialization must reject malformed engine/declaration structures and must not infer database identity from labels or frontend grammar.

### Frontend recovery

Publication recovery must rebuild database state from the ProjectIndex even when the current frontend database store is empty.

For each index database row:

- create or replace canonical declaration fields from the row;
- install the exact backend revision;
- install the backend-issued opaque resource path;
- create the corresponding resource projection;
- when a record with the same ID already exists, preserve only non-authoritative runtime enrichment such as columns, counts, and load state;
- authoritative name, engine, schema version, required flag, revision, and path always come from the index.

A database absent from the recovered index must be removed together with its revision and resource projection.

Recovery must not call a second database metadata command and must not merge two independently captured backend responses.

## 3. Strict resource-delta envelope

The runtime resource mutation validator must require the exact top-level key set:

```text
resource
fromRevision
toRevision
causedBy
payload
```

Unknown or missing top-level fields are invalid for every resource kind, including graph, function, variable, worksheet, and database.

Nested strict validation remains unchanged. This task must not broaden into an unrelated rewrite of all historical graph payload validators.

## 4. Testing strategy

All behavior changes use RED-GREEN TDD.

### Revisioned command snapshot

Add focused tests proving:

- project replacement triggered during authority read prevents IPC and publication;
- database, variable, and function-signature commands receive authority from the same lifecycle as `projectInstanceId` and `operationId`;
- missing revisions prevent IPC;
- normal receipts retain existing settlement and echo-deduplication behavior.

Tests must inject the lifecycle change at the authority-reader boundary rather than relying on timing or sleeps.

### Database recovery

Rust tests prove:

- ProjectIndex carries all canonical database declaration fields, exact revision, and opaque path;
- concurrent database authority changes cannot produce a mixed declaration/revision generation;
- serde/wire names match the frontend DTO exactly.

Frontend tests prove:

- an empty store materializes a missed database create;
- authoritative fields overwrite stale frontend values;
- runtime enrichment is preserved for an existing database;
- absent databases are removed from declaration, revision, and resource projections together;
- malformed or duplicate ProjectIndex database rows are rejected.

### Strict delta envelope

Table-driven tests prove:

- canonical database and non-database deltas are accepted;
- arbitrary top-level extra fields are rejected;
- omission of each required top-level field is rejected;
- existing nested database strictness remains intact.

## 5. Verification and review

Run focused tests for every changed module, then rerun all explicit Resource Catalog Tasks 1–8 frontend and Rust filters. Rust filters run serially with `CARGO_BUILD_JOBS=1`.

Required final gates:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

An independent task review must verify both spec compliance and code quality. A final whole-slice re-review must confirm that the three original blockers are addressed without introducing a second watermark, database event family, frontend resource synthesis, service-to-feature dependency, or direct view invocation.

After review and fresh controller verification, update `.superpowers/sdd/2026-08-03-revisioned-command-database-recovery/progress.md` and `TODO.md` under `## node_architecture 进度`.
