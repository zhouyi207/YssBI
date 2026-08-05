# Phase 1 Final Blockers Remediation Design

## Status

Approved on 2026-08-05.

This remediation removes the two load-bearing findings left by the Phase 1 whole-slice quality review. It does not reopen completed stable identity, Registry, legacy-runtime removal, or golden-contract behavior except where required to preserve their boundaries.

## Goals

1. Built-in node-system assembly propagates every construction failure through explicit `Result` values from fragment construction to Tauri setup.
2. No production thread-local error collector, fallback protocol, unvalidated semantic-ID constructor, `unwrap`, `expect`, `assert`, or panic is used to assemble built-in protocol, localization, or Registry inputs.
3. Editor projection DTO types, guards, parser, services, and feature consumers form a one-way runtime dependency graph with no cycle and no `services → features` dependency.
4. Existing wire formats, stable IDs, protocol fingerprints, Catalog behavior, editor projections, and project startup semantics remain unchanged for valid inputs.
5. Phase 1 is published as 100% only after task reviews, a clean whole-slice quality re-review, fresh `pnpm verify`, fixture read-only verification, and workspace hygiene checks.

## Non-goals

- Changing node identities, protocols, localization content, Registry fingerprint inputs, or provider ownership.
- Adding plugin loading, provider unloading, migration aliases, or fallback identities.
- Changing graph execution, relational execution, structured control, or resource path semantics.
- Replacing the Rust-to-TypeScript golden contract mechanism.
- Refactoring unrelated frontend domain or service modules.
- Preserving the thread-local/fallback assembly implementation as a compatibility layer.

## Typed built-in assembly

### Error model

Assembly uses one typed error chain. The exact variants may wrap existing identity and protocol errors, but must retain source information:

```rust
pub enum BuiltinAssemblyError {
    InvalidSemanticId {
        value: String,
        source: IdentityError,
    },
    InvalidProtocol {
        node_type: String,
        source: ProtocolError,
    },
    LocalizationConflict {
        locale: String,
        key: String,
    },
    Registration(NodeRegistrationError),
}
```

`BuiltinInitializationError` retains a typed assembly variant or a transparent typed conversion. Errors are never flattened to arbitrary strings before the public startup boundary.

### Required call graph

Every fallible assembly layer returns `Result`:

```text
fragment builder
  -> assemble_builtin_parts
  -> validate_builtin_bundle
  -> build_builtin_node_system
  -> ProjectStore::try_new
  -> ProjectState::try_new
  -> Tauri setup
```

No layer stores a failure out of band. The first error stops construction through `?` and no later fragment, Registry freeze, localization validation, store construction, state construction, or `app.manage` occurs.

### Semantic IDs

All semantic IDs use their existing validated constructors. Delete `AssemblySemanticId::from_unvalidated_assembly` and any equivalent raw constructor.

Static literals do not justify bypassing validation. A malformed literal must produce `BuiltinAssemblyError::InvalidSemanticId` through the same production assembly path used by valid literals.

### Protocol and fragment construction

Fragment builders return owned valid values only:

```rust
fn build_fragment(...) -> Result<ProviderFragment, BuiltinAssemblyError>
```

Helpers that construct ports, parameters, defaults, type expressions, schemas, execution semantics, or node protocols also return `Result` when their constructors can fail.

A helper must not return a fallback protocol, placeholder ID, empty port collection, default binding, or partially valid fragment after observing an error.

### Localization merge

Localization insertion returns a typed conflict when the same locale/key is supplied with a different value. Identical duplicate values may be accepted only if the existing contract explicitly permits idempotent merging.

Conflict detection must not use `assert_eq!`. The returned error identifies the locale and key without treating localized text as semantic identity.

### Startup boundary

`build_builtin_node_system()` remains the only production factory. Raw fragment/provider/catalog helpers remain private. Test injection uses narrowly scoped `#[cfg(test)]` functions that still call the real typed assembly/validation path rather than reimplementing it.

`ProjectStore::try_new`, `ProjectState::try_new`, and Tauri setup continue to propagate typed errors. No infallible production constructor is introduced.

## Editor projection dependency graph

### Module ownership

The shared DTO layer is split into one-way responsibilities:

```text
editorProjection.ts
  -> type declarations only

parameterEditorGuards.ts
  -> parameter-editor runtime validators

editorProjectionGuards.ts
  -> DTO structural validators
  -> imports DTO types with type-only imports
  -> imports parameter-editor validators

editorProjectionParser.ts
  -> structural guard
  -> graph/revision/ownership coherence validation

GraphProjectionService
  -> shared parser

feature/domain consumers
  -> shared DTO types/parser as needed
```

The exact parameter-editor validator filename may follow the existing shared DTO naming convention, but its runtime ownership must not create a return edge to `editorProjection.ts`.

### Forbidden dependencies

Production source audits and tests reject:

```text
services -> features
editorProjection.ts -> editorProjectionGuards.ts
editorProjectionGuards.ts -> editorProjection.ts at runtime
parser -> services
shared DTO -> views
```

Type-only imports are permitted where erased at runtime. Runtime re-exports that recreate a cycle are not permitted.

### Parsing behavior

`GraphProjectionService.loadGraph` and `hydrateGraph` continue to:

1. invoke the existing Tauri command with unchanged arguments;
2. receive `unknown`;
3. run exact structural validation;
4. run graph/revision/port/connection coherence validation;
5. return a validated `EditorGraphProjectionDto` or the existing sanitized public error.

Moving modules must not weaken exact-key checks, enum checks, fingerprint checks, safe-integer checks, finite-position checks, duplicate detection, endpoint ownership, or direction validation.

### Compatibility

Feature imports are migrated directly to shared modules. A temporary feature-level re-export is allowed only when it is type-only and creates no runtime edge; the preferred final state deletes the obsolete feature parser module.

No service imports from `features/` or `views/` remain.

## Testing strategy

### Typed assembly RED-GREEN

Focused tests inject through the real assembly path:

- invalid semantic ID;
- invalid protocol component;
- conflicting localization value;
- duplicate or invalid Registry input.

Each test asserts:

- exact typed variant and source;
- no panic via normal test execution rather than `catch_unwind` as the primary contract;
- no validated bundle returned;
- no ProjectStore or ProjectState returned;
- Tauri state management occurs only after success.

A source contract rejects production use of the deleted thread-local symbol, fallback constructors, unvalidated semantic-ID constructor, and assembly panic shortcuts.

Focused suites include built-in Catalog, Registry, ProjectStore, ProjectState startup, compiler, document, and golden contracts.

### DTO dependency RED-GREEN

Architecture tests inspect production imports and fail before the move on:

- service importing feature parser;
- DTO types runtime importing guards;
- guard/types/parser runtime cycles.

Behavior tests prove:

- authoritative editor fixture parses through the real service;
- every existing malformed projection case still fails;
- both service commands preserve names and arguments;
- parser public error remains unchanged;
- TypeScript typecheck succeeds.

### Final acceptance

After both tasks receive clean independent review, run focused checks and then exactly:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify
```

Recompute all five node-system fixture hashes before and after ordinary contract tests; hashes must remain identical. Run `git --no-pager diff --check` after progress publication.

A fresh whole-slice quality reviewer must report no Critical or Important findings. Deferred evidence/test-quality minors remain documented but do not weaken production contracts.

## Delivery constraints

Work remains on branch `shadcn`. Preserve user-authored commit `7c49163` and all unrelated dirty work. Do not create, amend, stage, commit, tag, push, reset, revert, or create a worktree.

This remediation owns a new SDD ledger. Update `TODO.md` after each clean reviewed task. Phase 1 remains 99% until the final quality review and fresh `pnpm verify` both pass.
