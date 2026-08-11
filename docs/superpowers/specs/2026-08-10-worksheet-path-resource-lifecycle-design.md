# Worksheet Path Resource Lifecycle Design

## Summary

Worksheets will use the same path-based resource identity and lifecycle model as
event and function graphs. A worksheet's canonical identity will be an opaque
resource path:

```text
worksheets/{validated-name}.yssbi-worksheet
```

The current UUID identity and UUID-based filename will be removed from the
production model. Creating, duplicating, saving, renaming, deleting, undoing,
redoing, publishing, and recovering worksheet changes will use the existing
revisioned resource mutation aggregate rather than the worksheet-specific delta
side channel.

This is a direct 0.x format replacement. The implementation will not retain a
legacy decoder, fallback scanner, dual identity, or migration shim for UUID
worksheet files or wire payloads.

## Goals

- Make worksheet filenames human-readable in the project directory.
- Make worksheet identity, revision, history, publication, and recovery behave
  consistently with graph resources.
- Use one opaque `worksheetPath` at every Rust, IPC, store, tab, and application
  boundary.
- Prevent invalid or non-portable resource names before filesystem mutation.
- Preserve zero-authoritative-effect behavior for stale requests, conflicts,
  and filesystem preparation failures.
- Keep frontend code from parsing, constructing, or interpreting resource
  paths.

## Non-goals

- Supporting nested worksheet directories.
- Migrating released projects or accepting UUID worksheet files.
- Retaining `worksheetId` as an alias for `worksheetPath`.
- Making worksheet documents graph documents or giving worksheets graph runtime
  projections.
- Adding a second worksheet-specific publication or resource-move protocol.

## Considered approaches

### Path identity matching graph resources — selected

Examples:

```text
events/Main.yssbi-event
functions/Calculate Revenue.yssbi-function
worksheets/Sales Report.yssbi-worksheet
```

The resource path is the sole identity. Renaming is a revisioned resource move.
This provides readable files and one coherent lifecycle model.

### Readable filename plus internal UUID — rejected

A filename such as `Sales Report--550e8400.yssbi-worksheet` would retain stable
UUID identity, but every boundary would need to choose between path and UUID.
That dual identity would preserve the current opportunity for identity drift.

### Name path plus UUID inside the document — rejected

Keeping path, document name, and UUID would create three potentially divergent
values and require compatibility rules for every mutation. It provides no
necessary capability for this 0.x project.

## Canonical identity and document format

Introduce a validated worksheet path type:

```rust
pub struct WorksheetResourcePath(String);
```

Its canonical serialized form is:

```text
worksheets/{validated-name}.yssbi-worksheet
```

`WorksheetDocument` will contain document state, not resource identity or a
second copy of the name:

```rust
pub struct WorksheetDocument {
    pub schema_version: u32,
    pub revision: ResourceRevision,
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
}
```

The following fields and concepts leave the production model:

```text
WorksheetDocument.id
WorksheetDocument.name
worksheetId
worksheets/{UUID}.yssbi-worksheet
```

Rust derives the display name from the validated path and returns it explicitly
in index and move DTOs. TypeScript treats `worksheetPath` as opaque and never
extracts or constructs a name from it.

The project worksheet index becomes equivalent to:

```rust
pub struct ProjectWorksheetIndexEntry {
    pub worksheet_path: WorksheetResourcePath,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub revision: ResourceRevision,
}
```

The duplicate `name` is acceptable at this DTO boundary because it is a Rust-
derived projection for display, not persisted identity.

## Shared resource-name validation

Event, function, and worksheet creation and rename must use one Rust-owned
validated type:

```rust
pub struct ResourceName(String);

impl ResourceName {
    pub fn parse(input: &str) -> Result<Self, ResourceNameError>;
}
```

Production resource paths may only be constructed from `ResourceName`. Commands,
frontend code, and filesystem transaction code must not independently format
resource paths from unchecked strings.

### Allowed characters

- Unicode letters.
- Unicode numbers.
- ASCII space.
- `-` and `_`.
- `(` and `)`.

### Rejected input

- `/`, `\\`, `:`, `*`, `?`, `"`, `<`, `>`, and `|`.
- Other punctuation and symbols, including emoji.
- Tabs, newlines, and all control characters.
- Leading or trailing spaces.
- Consecutive spaces.
- Empty names, `.` and `..`.
- Names ending in a period or space.
- Windows reserved names, case-insensitively: `CON`, `PRN`, `AUX`, `NUL`,
  `COM1` through `COM9`, and `LPT1` through `LPT9`.
- Names longer than 80 Unicode characters.
- Input that is not already in Unicode NFC canonical form.

Rust rejects invalid input instead of silently removing or replacing characters.
The frontend may provide immediate validation feedback or suggest normalized
input, but backend validation remains authoritative.

### Portable uniqueness

All platforms use the same case-insensitive, NFC-aware conflict check. `Report`,
`report`, and `REPORT` conflict even on a case-sensitive filesystem. This keeps
projects portable across Windows, macOS, and Linux.

Uniqueness is scoped by resource namespace. The following resources may coexist:

```text
events/Report.yssbi-event
functions/Report.yssbi-function
worksheets/Report.yssbi-worksheet
```

A case-only rename is valid when the source is the only conflicting resource.
On Windows, the filesystem transaction uses an internal temporary path so the
move does not depend on platform-specific case-only rename behavior. The
internal path never enters ProjectState, history, IPC, or publication.

## Unified mutation protocol

Worksheet mutations use the existing `ResourceMutationResultDto` returned by
graph lifecycle commands and emitted through
`ResourceMutationCommitted`.

The worksheet-specific side channel is removed:

```text
WorksheetDeltaDto
ResourceMutationResultDto.worksheet_deltas
ResourceMutationResultDto.worksheetDeltas
```

### Resource key

A worksheet is represented by its canonical path:

```rust
ResourceKey::Worksheet(
    WorksheetResourceKey("worksheets/Sales Report.yssbi-worksheet")
)
```

`WorksheetResourceKey` no longer contains a UUID.

### Generic lifecycle and move patches

The existing graph-specific lifecycle and move protocol will be generalized so
worksheets do not create a parallel implementation:

```rust
pub enum ResourceLifecycleKind {
    Event,
    Function,
    Worksheet,
}

pub struct ResourceLifecycleState {
    pub revision: ResourceRevision,
    pub path: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub name: String,
}

pub struct ResourceLifecyclePatch {
    pub before: Option<ResourceLifecycleState>,
    pub after: Option<ResourceLifecycleState>,
}
```

The resource patch union becomes equivalent to:

```rust
pub enum ResourceDocumentPatch {
    Graph(GraphDocumentPatch),
    Function(FunctionDocumentPatch),
    Worksheet(WorksheetDocumentPatch),
    ResourceLifecycle(ResourceLifecyclePatch),
    ResourceMove(ResourcePathMovePatch),
    Variable(VariableDocumentPatch),
    VariableScopeMove(ResourcePathMovePatch),
    Database(DatabaseDocumentPatch),
}
```

The generalized variants replace graph-only lifecycle and move variants; they
do not coexist as compatibility paths. `ResourceLifecycleState.name` is the
Rust-derived display name used by frontend create/delete projection updates, so
the frontend never parses a resource path. History resource moves are
generalized in the same way so worksheet create, rename, delete, undo, and redo
use the same durable lifecycle semantics as graph resources.

### Document delta

A worksheet content save publishes through the normal delta collection:

```rust
ResourceDeltaEvent {
    resource: ResourceKey::Worksheet(worksheet_path),
    from_revision,
    to_revision,
    caused_by,
    payload: ResourceDocumentPatch::Worksheet(patch),
}
```

### Resource move

`ResourceMoveDto.kind` expands from event/function to event/function/worksheet.
Worksheet rename is reported in `ResourceMutationResultDto.moves`, not a
worksheet-specific move object.

Create and duplicate use the same deterministic unique-name allocation for all
three resource namespaces: use the validated requested/base name when free,
otherwise select the first free `Name 2`, `Name 3`, and so on. Duplicate uses
the source display name as its base, so duplicating `Report` produces
`Report 2` when that is the first free name. Rename never silently changes the
requested name; a conflicting target returns `resource_name_conflict`.

## Commands

Worksheet commands align with graph lifecycle command identity and correlation:

```rust
create_worksheet(
    project_instance_id,
    worksheet_name,
    database_id: Option<String>,
    operation_id,
) -> ResourceMutationResultDto
```

```rust
duplicate_worksheet(
    project_instance_id,
    worksheet_path,
    expected_revision,
    operation_id,
) -> ResourceMutationResultDto
```

```rust
save_worksheet(
    project_instance_id,
    worksheet_path,
    expected_revision,
    operation_id,
    document,
) -> ResourceMutationResultDto
```

```rust
rename_worksheet_resource(
    project_instance_id,
    worksheet_path,
    expected_revision,
    new_name,
    lifecycle_token,
    operation_id,
) -> ResourceMutationResultDto
```

```rust
remove_worksheet(
    project_instance_id,
    worksheet_path,
    expected_revision,
    operation_id,
) -> ResourceMutationResultDto
```

As with the current graph commands, the lifecycle token is carried by rename,
while remove is guarded by project identity and expected resource revision.
Worksheet internals do not acquire graph runtime setup merely for symmetry;
only IPC identity, stale rejection, revisioning, filesystem mutation, history,
publication, and recovery behavior are shared.

Save and rename remain separate operations. `save_worksheet` cannot change the
name because the document contains no name. This prevents an ordinary save from
bypassing resource-move validation, history, or publication.

## Transaction behavior

### Create

1. Validate project lifecycle identity.
2. Parse the requested name through `ResourceName`.
3. Acquire the project filesystem root lease.
4. Recheck portable uniqueness under the lease.
5. Build the canonical worksheet path.
6. Prepare and commit the worksheet file.
7. Install ProjectState authority and revision state.
8. Record durable lifecycle history.
9. Return one `ResourceMutationResultDto` and emit it once through
   `ResourceMutationCommitted`.

The frontend installs the authoritative path returned by Rust and does not
predict it from the submitted name.

### Save

1. Validate project lifecycle identity and canonical worksheet path.
2. Validate the expected worksheet revision.
3. Prepare the content write at the canonical path.
4. Commit filesystem and ProjectState changes transactionally.
5. Record the worksheet document patch in history.
6. Publish the normal resource delta.

### Rename

1. Validate project lifecycle and worksheet lifecycle token.
2. Validate source existence and expected revision.
3. Parse and validate the new name.
4. Check target portable uniqueness under the root lease.
5. Capture the publication environment before filesystem commit.
6. Prepare and commit the filesystem move.
7. Move the ProjectState key from source path to destination path.
8. Preserve the resource revision domain and tombstone/ABA protection.
9. Record durable resource-move history.
10. Return and emit one correlated `ResourceMutationResultDto`.

Failures before filesystem commit have zero authoritative effects. A failure
after filesystem commit enters the existing authoritative publication recovery
path; the frontend must not synthesize rollback state.

### Delete

Delete follows `remove_graph` ordering:

- The request carries project identity, worksheet path, expected revision, and
  operation ID, matching `remove_graph`.
- The frontend does not close or unload the worksheet before the backend remove
  transaction commits.
- On success, publication removes the document and the frontend then closes or
  migrates affected temporary UI state.
- On failure, the tab, document, index, and filesystem remain unchanged.

This ordering prevents the unload/remove race previously found in graph
lifecycle operations.

## Frontend projection

The worksheet store remains domain-scoped but becomes path-keyed:

```ts
interface WorksheetStore {
  index: WorksheetIndexEntry[];
  documents: Record<string, WorksheetDocument>;
}

interface WorksheetIndexEntry {
  worksheetPath: string;
  name: string;
  databaseId: string;
  chartType: WorksheetChartType;
  revision: number;
}
```

The following frontend state uses the exact same opaque `worksheetPath` key:

- Loaded worksheet documents.
- Editor tab IDs and active tabs.
- Dirty, stale, and conflict registries.
- Pending operation correlation.
- Detail focus.
- Viewport, selection, and other temporary editor state.

The publication coordinator handles worksheet changes through `result.deltas`
and `result.moves`. It retains the existing direct-result/event-echo
suppression, publication revision ordering, and authoritative recovery gate.
Worksheets do not produce graph projection replacements because they have no
node graph projection.

For a worksheet move, the frontend migrates only path-keyed projection and
transient UI state. It does not rewrite backend-authoritative document content,
parse the path, or invent the destination name. The Rust-provided move and index
DTOs supply the display name.

## Error contract

Name and lifecycle failures use structured errors with stable codes:

```text
invalid_resource_name
resource_name_not_normalized
resource_name_reserved
resource_name_too_long
resource_name_conflict
resource_revision_conflict
stale_project_lifecycle
stale_resource_lifecycle
resource_not_found
filesystem_prepare_failed
filesystem_commit_failed
publication_recovery_required
```

A structured payload contains enough context for logs and UI messages:

```json
{
  "code": "resource_name_conflict",
  "message": "A worksheet named 'Report' already exists",
  "resourceKind": "worksheet",
  "resourcePath": "worksheets/report.yssbi-worksheet"
}
```

Frontend service and application layers use `formatErrorMessage`; structured
Tauri errors must never be logged as `[object Object]`.

## Persistence compatibility

Only canonical, flat worksheet paths are accepted. Project activation rejects:

- UUID-named worksheet files.
- A worksheet file whose path does not match its canonical resource path.
- Nested worksheet files.
- Symlinks, junctions, and reparse points.
- Case-folded duplicate worksheet paths.
- Invalid or non-NFC names.

There is no compatibility decoder, fallback scan, one-time migration, dual
write, or alias type. This follows the project's 0.x policy of removing legacy
paths directly.

## Testing

### Rust focused coverage

Name validation tests cover:

- Allowed Chinese and other Unicode letters, Latin letters, numbers, spaces,
  hyphens, underscores, and parentheses.
- Every forbidden filesystem character.
- Other punctuation, emoji, controls, leading/trailing spaces, and repeated
  spaces.
- Non-NFC input, reserved names, empty names, and overlong names.
- One shared validator used by event, function, and worksheet construction.

Filesystem and persistence tests cover:

- Human-readable canonical worksheet filenames.
- Documents without UUID identity or a duplicate persisted name.
- Rejection of old UUID filenames and noncanonical paths.
- Rejection of nested files and filesystem redirects.
- Portable case-fold collision behavior on every platform.
- Windows case-only rename through an internal temporary path.

Lifecycle tests cover:

- Create, duplicate, save, rename, and delete results.
- Undo and redo for lifecycle and document changes.
- Delete followed by same-path recreate with tombstone/ABA protection.
- Stale project and revision rejection with zero effects, plus stale lifecycle
  token rejection for rename.
- Concurrent create and rename collision handling under the root lease.
- Filesystem prepare, commit, rollback, and publication recovery failures.
- Delete ordering that does not unload before commit.

Rust-to-TypeScript golden contracts cover:

- Worksheet lifecycle delta.
- Worksheet document delta.
- Worksheet `ResourceMoveDto`.
- Direct mutation results and `ResourceMutationCommitted` envelopes.
- Undo and redo mutation results.

### TypeScript focused coverage

- Strict parsing of all canonical worksheet resource variants.
- Rejection of unknown fields, legacy `id`, and `worksheetDeltas`.
- Exactly-once application of direct results and correlated event echoes.
- Revision advancement on save.
- Move migration for store, tab, dirty, stale, conflict, and temporary UI keys.
- No frontend resource path parsing or reconstruction.
- Successful delete closes state only after authoritative publication.
- Failed delete preserves tabs and documents.
- Stale project results are rejected.
- Incomplete publication enters authoritative recovery.
- Project reload restores worksheet tabs by path.
- Sidebar display uses the Rust-provided name.
- Existing event and function behavior remains unchanged after protocol
  generalization.

### Test fixture version policy

Tests and golden fixture names must not contain the application software version.
Use semantic names such as:

```text
resource-mutation-worksheet-create.json
resource-mutation-worksheet-rename.json
resource-mutation-worksheet-delete.json
```

Do not use names such as `v0.2.7-worksheet-create.json` or assert that persisted
schema versions equal package or Cargo versions. `schemaVersion` remains a
persistence contract field and may be tested independently of application
release versions.

A legacy UUID field may appear only in a negative fixture proving strict
rejection. It must not establish a compatibility contract.

## Verification

Focused RED-GREEN tests precede behavior changes. Run project commands from the
repository root through `pnpm`:

```sh
pnpm typecheck
pnpm test
pnpm rust:fmt:check
pnpm rust:check
pnpm rust:test <focused-filter>
git diff --check
```

Because the implementation spans frontend and Rust, final delivery requires:

```sh
pnpm verify
```

Do not invoke ad-hoc Cargo commands that create `src-tauri/target/`. The existing
single Cargo build-job configuration remains the canonical Windows workflow.

## Completion criteria

The change is complete only when all of the following hold:

- Worksheet files have readable canonical names.
- `worksheetPath` is the sole worksheet identity at every production boundary.
- UUID worksheet identity has left the production type graph.
- Event, function, and worksheet names use one Rust validator.
- Worksheet mutations use common lifecycle, delta, move, history, publication,
  and recovery protocols.
- Create, rename, and delete failures preserve zero authoritative effects.
- Frontend code treats every resource path as opaque.
- Old UUID files and wire payloads are strictly rejected.
- Focused tests, relevant broader suites, `git diff --check`, and `pnpm verify`
  pass with fresh output.
