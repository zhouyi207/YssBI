# Resource Node Catalog and Documentation Design

## Goal

Extend the existing Static Catalog into one authoritative project-aware Catalog for function, variable, and database-backed DataFrame nodes, then route creation and documentation through exact backend-issued descriptors while removing legacy frontend inference.

## Authority model

Rust owns resource identity, revision, scope, protocol, localized documentation, descriptor eligibility, mutation validation, and parameter materialization. React caches and forwards descriptors unchanged; it never derives resource paths, revisions, parameters, or pins.

`MutationPublication.resource_revision` remains the sole public Catalog cache watermark. `MutationPublication.authority_generation` remains the internal coherence/CAS token. No second Catalog revision or event stream is introduced.

## Resource identities

Introduce `CatalogResourcePath` as an opaque serialized string. Canonical values are:

- function: `functions/...` canonical graph resource path;
- variable: `variables/{VariableId}`;
- database: `databases/{database-id}`.

The frontend may compare and forward these values but must not parse or construct them.

The existing UUID-backed `VariableId` remains authoritative. Update `AGENTS.md` to clarify that graph resources use `events/...` and `functions/...`, while UUIDs identify nodes, pins, connections, and variable resources. This is a documentation correction for existing architecture, not a persistence migration.

## Descriptor protocol

```rust
pub struct CatalogResourcePath(Box<str>);

pub enum NodeCreationDescriptor {
    Static {
        node_type_id: NodeTypeId,
    },
    ResourceBound {
        node_type_id: NodeTypeId,
        resource_path: CatalogResourcePath,
        resource_revision: ResourceRevision,
        create_args: ResourceBoundCreateArgsDto,
    },
}

pub enum ResourceBoundCreateArgsDto {
    Function,
    Variable,
    Database,
}
```

Wire fields use camelCase: `nodeTypeId`, `resourcePath`, `resourceRevision`, and `createArgs`. Database descriptors use the explicit `Database` variant rather than a generic resource variant.

Revision source:

- function: `FunctionDocument.revision`;
- variable: `ProjectState.variable_revisions[VariableId]`;
- database: the existing `ProjectState.database_authority_revisions[database_id]` token projected as the descriptor revision without adding another map.

## Canonical database publication

Extend the existing resource mutation model rather than introducing a database event family:

- add `ResourceKey::Database(DatabaseResourceKey)`;
- represent discovery-changing database commits in the canonical resource mutation receipt/delta payload;
- publish through the existing `ResourceMutationCommitted` project event;
- allocate exactly one `MutationPublication.resource_revision` for each successful discovery-changing database commit;
- advance the existing `database_authority_revisions` CAS entry in the same publication critical section;
- stale or failed operations change neither database authority, Catalog watermark, project event, nor authoritative data.

Every discovery-changing database IPC accepts caller-issued `projectInstanceId` and `operationId`. Create/import uses an expected-absent contract; rename/edit/save/delete also requires the exact expected database authority revision. Commands that currently return domain data return an aggregate DTO `{ data, mutation }`, where `data` preserves the existing `LoadDatabaseResult`/`EditState` payload and `mutation` is the canonical `ResourceMutationResultDto`. Commands with no domain payload return `{ data: null, mutation }`. The same mutation receipt is emitted once through `ResourceMutationCommitted`; no second receipt or follow-up query is introduced.

Database add/import, rename, schema/data identity edit, save, delete, and activation replacement receive focused tests. Activation reconstructs the database authority ledger from the incoming project and replaces the previous session state atomically.

## Coherent Catalog snapshot

`ProjectState::catalog_snapshot` produces:

```rust
pub struct CatalogProjectSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub resource_publication_revision: u64,
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub resources: Vec<CatalogResourceEntry>,
    authority_generation: u64,
}
```

Snapshot construction:

1. Capture project session, `resource_revision`, and `authority_generation`.
2. Hold the project filesystem lease while reading the project index and persisted unloaded function documents.
3. Overlay loaded graph/function documents, variable revisions/scopes, and database runtime entries from authoritative state snapshots.
4. Sort entries by stable opaque identity.
5. Release filesystem/state locks before localization.
6. Revalidate project session and `authority_generation` before returning.

Entries include function Call, variable Get/Set subject to scope, and database-backed `yssbi.dataframe.source.get` descriptors.

## Localized Catalog and documentation

Rust projects localized item title, description, documentation, aliases, category, icon/style, ports, and parameters with stable keys. Resource display names are localized/presentational only and never form identity.

Search indexes only title and aliases for the current locale. Documentation bodies and descriptions are excluded. Output remains deterministic under shuffled resource input.

The Catalog command returns exact snapshot metadata and localized resource/static items. The frontend uses the existing Catalog service/store, keyed by project instance, locale, registry fingerprint, and `resourcePublicationRevision`.

Project resource events invalidate/refetch Catalog state when the publication watermark advances. Stale responses cannot overwrite newer metadata.

## Two-stage mutation-time validation

Node creation wire shape becomes:

```rust
EditorGraphMutationDto::CreateNode {
    descriptor: NodeCreationDescriptor,
    position: NodePosition,
    user_label: Option<String>,
}
```

No arbitrary parameter map or compatibility field remains.

For resource descriptors:

1. Before entering `into_patch`, application/domain code captures an immutable resource validation snapshot under the required filesystem lease. It may read unloaded function documents and contains exact path, revision, scope, allowed node type, and permitted parameter binding.
2. I/O and filesystem leases end before mutation publication.
3. The publication critical section revalidates project instance, `authority_generation`, graph base revision, and descriptor tuple.
4. `into_patch` consumes only the immutable validation snapshot and performs no I/O.
5. It materializes only:
   - function Call: `target`;
   - variable Get/Set: `variable`;
   - database source: `dataframe`.
6. Normal protocol scope and parameter validation runs afterward.

Stale/missing resources return `catalog_resource_stale`. Malformed node/resource/create-args tuples, scope violations, and parameter injection return `catalog_descriptor_invalid`. Rejection has zero revision, history, event, graph, or filesystem effects.

## Frontend creation and refresh

The frontend descriptor type mirrors Rust exactly. `createNodeFromDescriptor`, palette, sidebar, DnD, and canvas drop forward descriptors unchanged.

`NodeSpawnTemplate` becomes `{ title?, descriptor }`. Variable/function/database overrides and parameter reconstruction are removed.

Shift-drop looks up the current descriptor by opaque resource path. If no exact current descriptor exists, it rejects creation, refreshes Catalog state, and shows the shared toast. It never synthesizes a fallback.

Duplicate/paste remain disabled until a separately specified descriptor-safe contract exists.

## Documentation UI

`NodeDocumentationModal` reads only the current Catalog hook/store and renders current-locale title/documentation, stable node ID, ports, parameters, and optional resource path/revision. It uses the existing Markdown renderer, shadcn primitives, shared toast behavior, and `OverlayScrollbar`. F1 behavior remains.

Empty/unavailable states are Catalog-derived; legacy unavailable placeholders and all-language documentation search are removed.

## Legacy removal boundaries

Remove only proven-dead Catalog-specific production paths:

- legacy Catalog `NodeDefinition` inference;
- contextual resource Catalog builders;
- frontend Call pin generation;
- variable/function/dataframe descriptor synthesis;
- all-locale documentation search;
- obsolete documentation placeholder.

Do not remove Rust-projected pin compatibility used by the canvas. Architecture audits are scoped to Catalog, docs, creation, and DnD paths rather than globally banning every `NodeDefinition` reference.

## Verification

Use focused serial Rust tests and explicit frontend Vitest files. Required gates are `pnpm typecheck`, `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check`. Do not run complete suites by default. Update `TODO.md` after every independently reviewed task.
