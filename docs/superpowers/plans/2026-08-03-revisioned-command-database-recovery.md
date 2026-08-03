# Revisioned Command and Database Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the remaining Resource Catalog blockers by atomically capturing revisioned command authority, making database recovery self-sufficient, and enforcing an exact resource-delta envelope.

**Architecture:** A generic application-layer snapshot helper captures project lifecycle before synchronously reading domain authority and revalidates it before IPC. Rust extends the already coherent ProjectIndex with canonical database declaration fields so frontend recovery needs no second read. The existing resource mutation validator applies one exact top-level shape to every resource delta.

**Tech Stack:** TypeScript, React/Zustand, Vitest, Rust, Serde, Tauri, pnpm.

## Global Constraints

- Work directly on `shadcn`; do not create a worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- Rust remains authoritative for project identity, resource declarations, revisions, and publication watermarks.
- `resource_revision` remains the sole public Catalog watermark.
- Treat database and other resource paths as opaque frontend values.
- Do not add a database-specific event, receipt, or public watermark.
- Frontend services must not import features; views must not call `invoke`.
- Do not hold global Rust locks during filesystem I/O.
- Use RED-GREEN TDD and focused tests. Run Rust filters serially with `CARGO_BUILD_JOBS=1`.
- Do not run complete frontend or Rust suites unless explicitly authorized.
- Update `.superpowers/sdd/2026-08-03-revisioned-command-database-recovery/progress.md` and `TODO.md` immediately after every independently reviewed task.

---

## File Structure

### Shared revisioned command snapshot

- Modify `src/features/application/projectCommandContext.ts`: own the generic lifecycle-plus-authority capture primitive.
- Modify `src/features/application/dataManagement/databaseMutation.ts`: consume the primitive for database revisions.
- Modify `src/features/application/dataManagement/variableActions.ts`: consume the primitive for variable revision/scope authority.
- Modify `src/features/application/editorMutation/functionSignatureCoordinator.ts`: consume the primitive for function revision/signature authority.
- Modify focused tests adjacent to those application modules; create `src/features/application/projectCommandContext.test.ts` if the helper lacks a direct test owner.

### Coherent database recovery

- Modify `src-tauri/src/project/project_io.rs`: extend `ProjectDatabaseIndexEntry` with canonical declaration fields.
- Modify `src-tauri/src/project/project_reads.rs`: project declaration and revision from one captured authority generation.
- Modify `src-tauri/src/commands/command_project/query.rs`: preserve the expanded DTO at the command boundary and test it.
- Modify `src/services/project/projectService.ts`: mirror the exact ProjectIndex database wire.
- Modify `src/features/application/editorMutation/projectPublicationRecovery.ts`: validate and materialize database declarations from ProjectIndex.
- Modify `src/features/core/dataStore/databaseStore.ts` only if a focused helper is required to separate canonical declaration from runtime enrichment.
- Modify `src/features/application/editorMutation/projectPublicationProductionStores.test.ts` and focused project-index tests.

### Strict resource-delta envelope

- Modify `src/features/core/sync/utils/resourceMutationWireValidator.ts`: require exact delta top-level keys.
- Modify `src/features/core/sync/utils/resourceMutationWireValidator.test.ts`: add positive and table-driven malformed cases.

---

### Task 1: Establish atomic revisioned command snapshots

**Files:**

- Modify: `src/features/application/projectCommandContext.ts`
- Modify: `src/features/application/dataManagement/databaseMutation.ts`
- Modify: `src/features/application/dataManagement/variableActions.ts`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.ts`
- Test: `src/features/application/projectCommandContext.test.ts`
- Test: `src/features/application/dataManagement/databaseMutation.test.ts`
- Test: existing focused variable/function coordinator tests discovered from the touched modules

**Interfaces:**

- Consumes: existing `captureProjectCommandContext()` and `ProjectCommandContext.assertCurrent()`.
- Produces:

```ts
export interface RevisionedProjectCommandSnapshot<T> {
  context: ProjectCommandContext;
  authority: T;
}

export function captureRevisionedProjectCommandSnapshot<T>(
  readAuthority: () => T,
): RevisionedProjectCommandSnapshot<T>
```

The exact exported name may be adjusted to existing naming style, but the helper must stay domain-agnostic and preserve the sequence capture → read → assert.

- [ ] **Step 1: Add RED helper and database race tests**

Inject a lifecycle replacement from inside the synchronous authority reader:

```ts
const readAuthority = () => {
  replaceProjectDuringAuthorityRead();
  return 7;
};

expect(() => captureRevisionedProjectCommandSnapshot(readAuthority)).toThrow();
expect(invokeDatabaseMutation).not.toHaveBeenCalled();
expect(submitPublication).not.toHaveBeenCalled();
```

Define `replaceProjectDuringAuthorityRead`, `invokeDatabaseMutation`, and `submitPublication` as focused test seams/mocks in the test file; they are not production APIs.

For `executeDatabaseMutation`, assert the database command and publication coordinator are not called when replacement occurs during revision acquisition.

- [ ] **Step 2: Run the RED tests**

Run:

```sh
pnpm test src/features/application/projectCommandContext.test.ts src/features/application/dataManagement/databaseMutation.test.ts
```

Confirm failure is caused by the absent atomic snapshot behavior, not test setup.

- [ ] **Step 3: Implement the generic helper**

The implementation must be equivalent to:

```ts
const context = captureProjectCommandContext();
const authority = readAuthority();
context.assertCurrent();
return { context, authority };
```

Do not import domain stores into `projectCommandContext.ts`.

- [ ] **Step 4: Migrate database mutations**

Replace revision-first capture with the helper. Preserve the existing aggregate `{ data, mutation }` settlement and operation identity behavior.

- [ ] **Step 5: Add RED variable and function-signature race tests**

For each path, trigger project replacement while reading its revision/signature authority and assert:

```text
Tauri command calls = 0
publication submissions = 0
optimistic authoritative mutations = 0
```

- [ ] **Step 6: Migrate variable and function-signature mutations**

Read all request-building authority needed by each mutation inside one callback result, for example:

```ts
const { context, authority } = captureRevisionedProjectCommandSnapshot(() => ({
  revision: readRevision(),
  signature: readSignature(),
}));
```

Do not widen the task into unrelated store refactors. Existing graph paths may adopt the helper only when mechanical and covered by their current tests.

- [ ] **Step 7: Run focused GREEN tests and static checks**

Run direct helper, database, variable, and function-signature tests, followed by:

```sh
pnpm typecheck
git diff --check
```

- [ ] **Step 8: Independent review and publication**

Reviewer must verify capture/read/assert ordering, zero effects on replacement, no service-to-feature dependency, and no behavior change in normal receipt settlement. After approval, append Task 1 evidence to the new ledger and update `TODO.md` Phase 3.

---

### Task 2: Make ProjectIndex database recovery self-sufficient

**Files:**

- Modify: `src-tauri/src/project/project_io.rs`
- Modify: `src-tauri/src/project/project_reads.rs`
- Modify: `src-tauri/src/commands/command_project/query.rs`
- Modify: `src/services/project/projectService.ts`
- Modify: `src/features/application/editorMutation/projectPublicationRecovery.ts`
- Test: focused Rust tests in `project_reads.rs` and `command_project/query.rs`
- Test: `src/features/application/editorMutation/projectPublicationProductionStores.test.ts`
- Test: focused ProjectIndex validation/recovery tests discovered from `projectPublicationRecovery.ts`

**Interfaces:**

- Consumes: existing coherent `read_project_index_with` capture/revalidation and `database_authority_revisions`.
- Produces an exact database index row equivalent to:

```ts
export interface ProjectDatabaseIndexRow {
  id: string;
  resourcePath: string;
  revision: number;
  engine: DatabaseEngineDto;
  schemaVersion: number;
  required: boolean;
  name: string | null;
}
```

Use the repository's actual `DatabaseEngine`/DTO type names and Serde casing.

- [ ] **Step 1: Add RED Rust ProjectIndex projection tests**

Construct a database declaration with non-default engine, schema version, required flag, and nullable/name cases. Assert the index row carries those exact fields together with opaque path and exact revision.

Add a coherence seam test that permits a database mutation during capture and accepts only a complete before-generation or after-generation row, never mixed declaration/revision facts.

- [ ] **Step 2: Run RED Rust filters serially**

Run the exact `project_reads` and `command_project::query` tests with `CARGO_BUILD_JOBS=1` and `--test-threads=1`. Confirm the new declaration assertions fail against the current row.

- [ ] **Step 3: Extend the Rust DTO and coherent projection**

Project fields directly from the captured `ProjectData.databases` declaration and pair them with the captured revision map. Do not perform filesystem I/O or a second state read during overlay.

- [ ] **Step 4: Add RED frontend recovery tests**

Cover:

```text
empty store + index row → database declaration, revision, resource all created
stale canonical fields → index fields win
existing runtime enrichment → columns/count/load state survive
missing index row → declaration, revision, resource all removed
```

Add malformed row cases for missing declaration fields, malformed engine, empty ID/path, invalid revision, and duplicate IDs.

- [ ] **Step 5: Run RED frontend recovery tests**

Confirm missed-create materialization fails because current recovery only retains an existing frontend database.

- [ ] **Step 6: Implement strict frontend DTO validation and recovery**

Build a canonical database record from every index row. Merge only explicitly non-authoritative runtime enrichment from an existing same-ID record. Replace database declarations, revision map, and resource projection from the same prepared recovery commit.

Do not call `get_project_databases_variables` or any second IPC during recovery.

- [ ] **Step 7: Run focused GREEN tests and gates**

Run the focused frontend recovery tests and serial Rust projection/query filters, then:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 8: Independent review and publication**

Reviewer must verify one-generation declaration/revision coherence, missed-create recovery from an empty store, runtime enrichment boundaries, opaque paths, deletion symmetry, strict wire validation, and absence of a second recovery read. After approval, append Task 2 evidence and update `TODO.md` Phase 3/9.

---

### Task 3: Enforce the exact resource-delta envelope

**Files:**

- Modify: `src/features/core/sync/utils/resourceMutationWireValidator.ts`
- Modify: `src/features/core/sync/utils/resourceMutationWireValidator.test.ts`

**Interfaces:**

- Consumes: existing `hasExactKeys` and `areResourceDeltasValid`.
- Produces: exact envelope validation for `resource`, `fromRevision`, `toRevision`, `causedBy`, and `payload` across every resource kind.

- [ ] **Step 1: Add RED table-driven malformed-wire tests**

Starting from canonical database and variable/graph deltas, assert rejection for:

```ts
{ ...canonicalDelta, unexpected: true }
```

and for deletion of each required top-level key.

- [ ] **Step 2: Run the RED validator test**

Confirm the extra-field cases currently pass validation and therefore make the test fail for the expected reason.

- [ ] **Step 3: Add the minimal exact-key guard**

Immediately after `isRecord(value)`, require:

```ts
hasExactKeys(value, [
  'resource',
  'fromRevision',
  'toRevision',
  'causedBy',
  'payload',
])
```

Retain existing branch-specific nested validators. Do not rewrite unrelated graph payload validation.

- [ ] **Step 4: Run GREEN validator and publication tests**

Run the validator test plus focused `ProjectMutationEventHandler` and publication result tests to ensure canonical events still pass.

- [ ] **Step 5: Run static checks**

```sh
pnpm typecheck
git diff --check
```

- [ ] **Step 6: Independent review and publication**

Reviewer must verify exact top-level rejection applies to all resource kinds without changing legal nested payloads. After approval, append Task 3 evidence and update `TODO.md` Phase 9.

---

### Task 4: Final focused verification and whole-slice review

**Files:**

- Modify: `.superpowers/sdd/2026-08-03-revisioned-command-database-recovery/progress.md`
- Modify: `TODO.md`

- [ ] **Step 1: Run every explicit frontend file from Tasks 1–3**

Use one `pnpm test file1 file2 ...` command without an extra `--` separator. Include the Resource Catalog Tasks 5–7 explicit frontend files and the database publication/search tests changed by the previous final fix wave.

- [ ] **Step 2: Run focused Rust filters serially**

Include the ProjectIndex/database filters from Task 2 and all explicit Resource Catalog Tasks 1–4 and Task 7 filters. Set `CARGO_BUILD_JOBS=1` and `--test-threads=1` for every test command.

- [ ] **Step 3: Run final static gates**

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 4: Dispatch an independent whole-slice reviewer**

The reviewer must explicitly adjudicate the original blockers:

```text
project identity and revision are captured from one lifecycle
missed database create is recoverable from one coherent ProjectIndex
resource delta top-level extras are rejected
```

It must also check no second watermark/event family, no frontend path synthesis, no service-to-feature dependency, and no direct view invocation.

- [ ] **Step 5: Publish completion only with fresh controller evidence**

Append exact test counts and review verdict to the ledger. Raise `TODO.md` percentages only when the whole-slice review has no Critical or Important findings. Do not claim complete based only on subagent reports.
