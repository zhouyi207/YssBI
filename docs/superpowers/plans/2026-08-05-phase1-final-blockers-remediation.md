# Phase 1 Final Blockers Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the final two Phase 1 quality blockers by making built-in assembly a fully typed `Result` chain and making editor projection dependencies strictly one-way, then publish Phase 1 at 100% only after clean independent reviews and fresh verification.

**Architecture:** Rust built-in construction becomes an ordinary fail-fast call graph: validated semantic IDs and protocol helpers return `Result`, each provider fragment propagates `?`, Registry/localization validation finishes before `ProjectStore`, `ProjectState`, or Tauri management. Frontend projection ownership becomes `editorProjection.ts` declarations → `parameterEditorValidators.ts` runtime validators → `editorProjectionGuards.ts` structural guard → `editorProjectionParser.ts` coherence parser → `GraphProjectionService`, with feature consumers importing shared parser/types directly and no reverse edge.

**Tech Stack:** Rust 2024, Serde/serde_json, Tauri 2, TypeScript 5.8, Vitest 4, pnpm 10, shadcn/ui.

## Global Constraints

- Work directly on existing branch `shadcn`; do not create or switch a branch or worktree.
- Preserve user-authored commit `7c4916381d07e3f2c11421cc31f4aa3657007613` and all unrelated dirty work. Do not reset, revert, restore, clean, amend, or overwrite it.
- Do not stage, commit, tag, or push. This plan contains no commit step.
- Before every task and review, record `git --no-optional-locks status --short --branch` and `git --no-pager diff --stat`; compare them with the captured baseline and restrict edits to that task's file map.
- Use direct shadcn/ui primitives for any interactive UI touched by follow-up fixes; do not add another UI library. This remediation is not expected to change UI production files.
- Rust remains authoritative for protocol, stable IDs, Registry inputs, provider provenance, localization inventory, project state, and golden fixture generation.
- Preserve valid-input behavior exactly: no node ID, protocol, localization text, wire shape, fingerprint, Catalog behavior, editor projection, Tauri command name/argument, or startup semantic change.
- Do not add aliases, migration shims, fallback IDs/protocols, compatibility re-exports that create runtime edges, dual parsing paths, or a second built-in factory.
- `build_builtin_node_system()` remains the only production built-in factory. Test injection must be `#[cfg(test)]`, narrowly scoped, and must traverse the same typed assembly and validation functions.
- Production built-in assembly must not use thread-local/global error collectors, fallback/placeholder values, `AssemblySemanticId`, `from_unvalidated_assembly`, raw semantic-ID construction, `unwrap`, `expect`, `assert`, `assert_eq!`, `panic!`, or ignored `Result` values.
- Every fallible layer in `fragment builder → assemble_builtin_parts → validate_builtin_bundle → build_builtin_node_system → ProjectStore::try_new → ProjectState::try_new → Tauri setup` returns and propagates `Result`; the first error prevents every later layer and `app.manage(project_state)`.
- Frontend production dependencies must be one-way: types → parameter validators → structural guards → coherence parser → service → consumers. Type-only imports are erased and allowed; runtime re-exports that recreate a cycle are forbidden.
- Production imports `services → features`, `editorProjection.ts → editorProjectionGuards.ts`, `editorProjectionGuards.ts → editorProjection.ts` at runtime, `parser → services`, and `shared DTO → views` are forbidden.
- Exact structural checks, enum checks, lowercase 64-character fingerprint checks, non-negative safe-integer checks, finite positions, JSON-number checks, duplicate detection, graph/revision coherence, port ownership, endpoint existence, and endpoint direction checks must remain unchanged.
- The public malformed projection error remains exactly `Invalid editor graph projection response`; `load_project_graph` and `hydrate_editor_graph` command names and argument objects remain unchanged.
- Use RED-GREEN for Tasks 1 and 2: add focused failing tests first, run the named RED command and record the expected failure, implement only enough to satisfy the contracts, then run the full task GREEN matrix.
- Run focused Rust tests from the repository root with `CARGO_BUILD_JOBS=1` and `--test-threads=1`; do not invoke Cargo from `src-tauri` and do not run the full Rust suite before Task 3.
- For Rust production changes run `CARGO_BUILD_JOBS=1 pnpm rust:check` and `pnpm rust:fmt:check`. For frontend production changes run exact Vitest files and `pnpm typecheck`.
- Run `git --no-pager diff --check` after every GREEN task and after final publication.
- Each task requires a fresh independent reviewer. Any Critical or Important finding makes the review non-clean; resolve it through a new focused RED-GREEN iteration, rerun the task GREEN matrix, and obtain a new independent review.
- Create `.superpowers/sdd/2026-08-05-phase1-final-blockers-remediation/progress.md` only after Task 1 has a clean review. Append later task evidence only after that task's clean review.
- Ledger entries use this exact shape: task number/title; baseline status; reviewed diff paths; RED command and observed failure; GREEN commands and observed counts; independent reviewer verdict; findings and resolutions; fixture hashes when applicable; contracts handed to the next task.
- Modify only the Phase 1 row under `TODO.md`'s `## node_architecture 进度`, and only after a clean reviewed task. Keep Phase 1 at 99% after Tasks 1 and 2.
- Phase 1 reaches 100% only after Task 3 has two clean whole-slice reviews, all focused checks pass, all five fixture hashes remain identical, fresh `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify` passes, and final workspace hygiene passes.
- Do not edit the original ledger `.superpowers/sdd/2026-08-04-phase1-registry-identity-closure/progress.md` or its `final-quality-fix-report.md`; they are immutable input evidence for this remediation.

---

## File Responsibility Map

### Task 1 — Typed built-in assembly

- Modify `src-tauri/src/node_system/protocol/identity.rs`: delete `AssemblySemanticId` and `from_unvalidated_assembly`; retain `InvalidSemanticId` and validated `new`/`FromStr` paths as the only constructors.
- Modify `src-tauri/src/node_system/protocol/mod.rs`: remove the deleted assembly trait export and expose only the existing validated identity/error types needed by catalog assembly.
- Modify `src-tauri/src/node_system/catalog/builtin.rs`: own `BuiltinAssemblyError`, fallible semantic/protocol helpers, fragment merge/finalization, typed test fault injection, assembly, Registry validation, and the sole production factory.
- Modify `src-tauri/src/node_system/catalog/control.rs`: make control registration/protocol/port/parameter builders return `Result` and propagate `?`.
- Modify `src-tauri/src/node_system/catalog/core_nodes/mod.rs`, `core_nodes/support.rs`, `core_nodes/control.rs`, `core_nodes/debug.rs`, `core_nodes/math.rs`, and `core_nodes/value.rs`: make core fragment registration and shared semantic/protocol helpers fallible without changing emitted values.
- Modify `src-tauri/src/node_system/catalog/core_nodes/coverage_tests.rs`: unwrap only at the test boundary and retain exact migrated-node coverage.
- Modify `src-tauri/src/node_system/catalog/dataframe/mod.rs` and `dataframe/tests.rs`: make dataframe fragment/protocol/port construction fallible. Read but do not modify `dataframe/families.rs`; its declarative family definitions do not construct protocol values.
- Modify `src-tauri/src/node_system/catalog/distribution/mod.rs`, `plot/mod.rs`, `project.rs`, and `statistics/mod.rs`: make fragment/protocol/port/parameter construction fallible and preserve protocols byte-for-byte for valid literals.
- Modify `src-tauri/src/node_system/catalog/statistics/tests.rs`: adapt test-boundary fragment construction to `Result` without weakening assertions.
- Modify `src-tauri/src/node_system/catalog/localization.rs`: keep `BuiltinCatalog::new(...) -> Result<_, ProtocolError>` and ensure assembly maps its source without panic or text flattening.
- Modify `src-tauri/src/node_system/catalog/mod.rs`: export the final typed errors/factory and only `#[cfg(test)]` fault-injection surface.
- Modify `src-tauri/src/node_system/catalog/tests.rs`: add exact typed invalid-ID, invalid-protocol, localization-conflict, duplicate-registration, fail-fast, and valid-behavior tests.
- Modify `src-tauri/src/node_system/testing/source_audit.rs`: add a production assembly contract that rejects the deleted collector, raw constructor, fallbacks, panic shortcuts, and ignored assembly results.
- Modify `src-tauri/src/project/project_store.rs`: retain `try_new`; make test injection consume a typed built-in factory result and prove no store is returned on assembly/registration failure.
- Modify `src-tauri/src/project/project_state.rs`: retain `try_new`; add narrowly scoped test construction from a fallible store factory and prove no state is returned on failure.
- Modify `src-tauri/src/lib.rs`: extract/test the state-construction-before-manage boundary without changing Tauri setup order or later setup work.
- Modify `src-tauri/src/node_system/compiler/task1_tests.rs`, `src-tauri/src/node_system/document/tests.rs`, and `src-tauri/src/node_system/registry/tests.rs`: terminate the new fallible test builders with explicit test-boundary `expect`/`unwrap` while preserving all assertions.

### Task 2 — One-way editor projection modules

- Modify `src/shared/types/dto/editorProjection.ts`: declarations only; delete runtime validator implementation and runtime guard re-export.
- Create `src/shared/types/dto/parameterEditorValidators.ts`: own `isSchemaAwareParameterEditorDto(value: unknown): value is SchemaAwareParameterEditorDto` and its private exact-key/column/literal/predicate validators using type-only imports.
- Modify `src/shared/types/dto/editorProjectionGuards.ts`: import DTOs with `import type`, import the parameter validator at runtime, and retain complete exact structural validation.
- Modify `src/shared/types/dto/editorProjectionParser.ts`: import DTOs type-only and guards/parameter validator at runtime; retain structural-first parsing and all coherence validation/error text.
- Modify `src/shared/types/dto/index.ts`: export projection declarations only through `editorProjection`; do not export guards, parser, or parameter validators from the barrel. Runtime consumers use their explicit module paths.
- Delete `src/features/domain/editorProjection/validateProjection.ts`: remove the obsolete feature-level parser shim.
- Modify `src/features/domain/editorProjection/index.ts`: stop exporting the deleted shim.
- Modify `src/features/domain/editorProjection/toProjectionEntities.ts`: import `validateEditorGraphProjection` directly from the shared parser.
- Modify `src/features/domain/editorProjection/editorProjection.test.ts`: import the shared parser directly and expand the architecture contract to all five shared/service layers.
- Modify `src/services/nodeSystem/graphProjectionService.test.ts`: prove both real service methods preserve command names/arguments, accept the authoritative fixture, sanitize malformed values, and contain no feature imports.
- Modify `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`: import guards from `editorProjectionGuards`, parameter validation from `parameterEditorValidators`, and parser from `editorProjectionParser`; retain every malformed mutation.
- Read but do not modify `src/tests/helpers/editorProjectionFixtures.ts`; its DTO type-only imports remain valid and its fixture values/wire shape are acceptance evidence.

### Task 3 — Acceptance evidence and Phase 1 publication

- Review all paths changed by Tasks 1 and 2 plus the protected dirty baseline.
- Read-only verify `src/tests/fixtures/node-system-contracts/editor-projection.json`, `fingerprint-wire.json`, `i18n-inventory.json`, `localized-catalog.json`, and `semantic-protocol.json`.
- Create after Task 1 clean review and append after later clean reviews: `.superpowers/sdd/2026-08-05-phase1-final-blockers-remediation/progress.md`.
- Modify after each clean review only: `TODO.md` Phase 1 row.
- Modify for execution checkbox tracking only: `docs/superpowers/plans/2026-08-05-phase1-final-blockers-remediation.md`.
- Do not modify production code in Task 3. A blocker found here reopens Task 1 or Task 2.

---

### Task 1: Replace built-in assembly escape hatches with typed `Result`

**Files:**
- Modify: all Task 1 files in the File Responsibility Map.
- Test: `src-tauri/src/node_system/catalog/tests.rs`
- Test: `src-tauri/src/node_system/testing/source_audit.rs`
- Test: `src-tauri/src/project/project_store.rs`
- Test: `src-tauri/src/project/project_state.rs`
- Test: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: existing validated constructors such as `NodeTypeId::new`, `I18nKey::new`, `PortKey::new`, `ParameterKey::new`, `NodeInterfaceProtocol::new`, `NodeInterfaceProtocol::with_member_groups`, `ParameterSchema::new`, `NodeRegistryBuilder::register_provider`, `NodeRegistryBuilder::freeze`, and `BuiltinCatalog::validate`.
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinAssemblyError {
    InvalidSemanticId {
        value: Box<str>,
        source: InvalidSemanticId,
    },
    InvalidProtocol {
        node_type: Box<str>,
        source: ProtocolError,
    },
    LocalizationConflict {
        locale: Box<str>,
        key: Box<str>,
    },
    Registration(NodeRegistrationError),
}

pub enum BuiltinInitializationError {
    Assembly(BuiltinAssemblyError),
    Localization(I18nBundleValidationError),
}

pub(crate) fn assembled_interface(
    node_type: &str,
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    type_constraints: Vec<TypeConstraint>,
    member_groups: Vec<PortMemberGroupSpec>,
) -> Result<NodeInterfaceProtocol, BuiltinAssemblyError>;

pub(crate) fn assembled_parameters(
    node_type: &str,
    parameters: Vec<ParameterSpec>,
) -> Result<ParameterSchema, BuiltinAssemblyError>;

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError>;

pub fn build_builtin_node_system(
) -> Result<BuiltinNodeSystem, BuiltinInitializationError>;

pub fn ProjectStore::try_new(
) -> Result<ProjectStore, BuiltinInitializationError>;

pub fn ProjectState::try_new(
) -> Result<ProjectState, BuiltinInitializationError>;
```

- `BuiltinAssemblyError::source()` returns the exact `InvalidSemanticId`, `ProtocolError`, or `NodeRegistrationError`; localization conflict has no nested source and retains exact locale/key.
- Semantic helper contract: each literal calls its existing `new` constructor and maps failure to `InvalidSemanticId { value, source }`; no helper can construct the tuple field directly.
- Protocol helper contract: callers pass the owning stable node type string so `InvalidProtocol { node_type, source }` identifies the failed protocol without using localized text.
- Fragment contract: `merge` consumes only successful `ProviderFragment` values; `finish` validates message IDs and conflicting locale/key pairs and returns no partial fragment.
- Test-only contract:

```rust
#[cfg(test)]
pub(crate) enum BuiltinAssemblyTestFault {
    InvalidSemanticId(&'static str),
    InvalidProtocol(&'static str),
    LocalizationConflict,
    DuplicateRegistration,
}

#[cfg(test)]
pub(crate) fn build_builtin_node_system_with_test_fault(
    fault: BuiltinAssemblyTestFault,
) -> Result<BuiltinNodeSystem, BuiltinInitializationError>;
```

This function injects one malformed input into `assemble_builtin_parts_with(...)` and then calls the same `validate_builtin_bundle(...)`; it must not duplicate assembly or validation logic.

- [ ] **Step 1: Capture the protected baseline**

Run:

```sh
git --no-optional-locks status --short --branch
git --no-pager diff --stat
git --no-pager diff -- src-tauri/src/node_system/catalog src-tauri/src/node_system/protocol/identity.rs src-tauri/src/node_system/protocol/mod.rs src-tauri/src/project/project_store.rs src-tauri/src/project/project_state.rs src-tauri/src/lib.rs
```

Expected: branch `shadcn`, HEAD `7c49163`, and the existing dirty files from the prior Phase 1 wave remain present. Save the output in working notes; do not stage anything.

- [ ] **Step 2: Write RED typed assembly tests**

In `catalog/tests.rs`, add four tests using `build_builtin_node_system_with_test_fault`:

```rust
#[test]
fn builtin_assembly_rejects_invalid_semantic_id_with_source() {
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidSemanticId("Bad Display ID"),
    ).unwrap_err();
    assert!(matches!(
        error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::InvalidSemanticId { ref value, ref source }
        ) if value.as_ref() == "Bad Display ID"
            && source.to_string().contains("invalid node type id")
    ));
}

#[test]
fn builtin_assembly_rejects_invalid_protocol_without_fallback() {
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidProtocol("yssbi.test.invalid_protocol"),
    ).unwrap_err();
    assert!(matches!(
        error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::InvalidProtocol { ref node_type, .. }
        ) if node_type.as_ref() == "yssbi.test.invalid_protocol"
    ));
}

#[test]
fn builtin_assembly_rejects_conflicting_localization() {
    assert!(matches!(
        build_builtin_node_system_with_test_fault(
            BuiltinAssemblyTestFault::LocalizationConflict,
        ),
        Err(BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::LocalizationConflict { ref locale, ref key }
        )) if locale.as_ref() == "en-US" && key.as_ref() == "nodes.test.title"
    ));
}

#[test]
fn builtin_assembly_rejects_duplicate_registration() {
    assert!(matches!(
        build_builtin_node_system_with_test_fault(
            BuiltinAssemblyTestFault::DuplicateRegistration,
        ),
        Err(BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::Registration(_)
        ))
    ));
}
```

Use ordinary test execution as the no-panic assertion; do not use `catch_unwind` as the primary contract.

- [ ] **Step 3: Write RED fail-fast startup tests**

Add tests beside `ProjectStore`, `ProjectState`, and the setup helper. Inject a closure returning the Task 1 typed error and increment `AtomicUsize` counters in the later constructor/manage closure. Assert the result is the exact typed error and every later counter is zero. The production helper called by Tauri must have this shape:

```rust
fn initialize_project_state(
) -> Result<ProjectState, node_system::catalog::BuiltinInitializationError> {
    ProjectState::try_new()
}
```

The setup sequence remains:

```rust
let project_state = initialize_project_state().map_err(Box::<dyn std::error::Error>::from)?;
app.manage(project_state);
```

- [ ] **Step 4: Extend the RED production source contract**

Add `builtin_assembly_has_no_escape_hatches` to `source_audit.rs`. Scan production Rust sources under `node_system/catalog`, `node_system/protocol/identity.rs`, `project/project_store.rs`, `project/project_state.rs`, and `lib.rs`; exclude `#[cfg(test)]` items through the existing AST machinery. Reject exact symbols/tokens `ASSEMBLY_PROTOCOL_ERROR`, `record_protocol_error`, `run_assembly`, `AssemblySemanticId`, `from_unvalidated_assembly`, fallback protocol/schema struct literals after a failed constructor, and assembly-path `unwrap`/`expect`/`assert`/`assert_eq`/`panic` or discarded `Result`.

- [ ] **Step 5: Run RED and record the expected failures**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests::builtin_assembly_rejects_ -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::source_audit::builtin_assembly_has_no_escape_hatches -- --exact --test-threads=1
```

Expected: compile/test failure because `BuiltinAssemblyTestFault` and `build_builtin_node_system_with_test_fault` do not exist, current error variants differ, and the audit finds the thread-local collector/raw constructor/fallback helpers. Record the actual failure, not only the expectation.

- [ ] **Step 6: Delete raw semantic-ID construction**

Remove `AssemblySemanticId` and its macro-generated implementation from `protocol/identity.rs`, remove its export/imports, and route every catalog semantic literal through `Type::new(...)`. Map `InvalidSemanticId` without converting it to `String`.

- [ ] **Step 7: Implement the typed error chain and fallible primitive helpers**

In `builtin.rs`, implement the exact interfaces above. Delete `RefCell`, the thread-local slot, `record_protocol_error`, `run_assembly`, and both fallback values. Implement `Display`, `Error::source`, and `From<BuiltinAssemblyError> for BuiltinInitializationError`; map Registry registration/freeze errors to `BuiltinAssemblyError::Registration` before localization validation.

- [ ] **Step 8: Convert built-in and control construction to `Result`**

Make `protocol`, `constant_protocol`, `equality_protocol`, `data_port_expr`, `iid`, and semantic helper calls in `builtin.rs` return/propagate `Result`. Make `control::register`, `protocol`, `port`, and `parameter` fallible. Preserve every valid field value and ordering; use `?` at each constructor boundary.

- [ ] **Step 9: Convert core node fragments to `Result`**

Change `core_nodes::build_provider_fragment` and each `register` function in `core_nodes/{control,debug,math,value}.rs` to return `Result<(), BuiltinAssemblyError>` or `Result<ProviderFragment, BuiltinAssemblyError>`. Change `support::{protocol,parameter}` and its semantic/i18n/port helpers to return `Result`. Update coverage tests to call `.expect("core built-in fixture must assemble")` only at the test boundary.

- [ ] **Step 10: Convert dataframe/statistics/distribution/plot/project fragments**

For each exact module in the Task 1 map, convert `build_provider_fragment`, `registered_node`, `protocol`, `port`, `parameter`, and key helpers that invoke fallible constructors to `Result` and use `collect::<Result<Vec<_>, _>>()?` for iterator construction. Do not alter the family tables, literals, sorting, node count, or registration order.

- [ ] **Step 11: Make fragment merge/finalization fail fast**

Change assembly to call each fragment sequentially with `?`, then `fragment.merge(successful_fragment)`. `ProviderFragment::finish` must validate each message key with `I18nKey::new`, accept identical duplicate locale/key values, return `LocalizationConflict` for different values, and never return a partial fragment.

- [ ] **Step 12: Add narrow real-path fault injection**

Implement `BuiltinAssemblyTestFault` and `build_builtin_node_system_with_test_fault` under `#[cfg(test)]`. Invalid ID enters a real fragment constructor, invalid protocol enters `NodeInterfaceProtocol::new`/`ParameterSchema::new`, localization conflict enters `ProviderFragment::finish`, and duplicate registration enters `validate_builtin_bundle`. Delete the older `build_builtin_node_system_with_test_fragment` API and replace its `catalog/mod.rs` test export with the fault-injection API.

- [ ] **Step 13: Complete ProjectStore/ProjectState/setup propagation**

Keep production `ProjectStore::try_new` and `ProjectState::try_new` signatures unchanged. Add only private/`#[cfg(test)]` closure-based helpers needed by Step 3; verify failed assembly cannot construct or return store/state and `app.manage(project_state)` remains after successful `initialize_project_state()?`.

- [ ] **Step 14: Run focused GREEN assembly/startup suites**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::source_audit -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::task1_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::document::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_store::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_state::startup_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::contracts -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git --no-pager diff --check
```

Expected: all commands pass; valid Catalog/Registry/compiler/document/golden behavior is unchanged; production check has no newly introduced warnings.

- [ ] **Step 15: Run direct forbidden-symbol and behavior-preservation audits**

Run:

```sh
git --no-pager grep -n "ASSEMBLY_PROTOCOL_ERROR\|record_protocol_error\|run_assembly\|AssemblySemanticId\|from_unvalidated_assembly" -- "src-tauri/src/**/*.rs"
git --no-pager grep -n "fallback" -- "src-tauri/src/node_system/catalog/**/*.rs" "src-tauri/src/node_system/protocol/identity.rs"
```

Expected: no production matches. Test-only negative token lists may match only inside `source_audit.rs`. Compare generated fixture diff with the baseline; no fixture may change.

- [ ] **Step 16: Obtain a clean independent Task 1 review**

Give a fresh reviewer the approved design, original final-quality report, Task 1 diff only, RED/GREEN output, forbidden-symbol output, and baseline status. Require explicit findings by severity for typed source preservation, real validated IDs, no collector/fallback/panic, complete `Result` propagation, fail-fast ordering, private test injection, startup-before-manage, and valid behavior preservation. Resolve every Critical/Important finding through a new focused failing regression, rerun Steps 14-15, and repeat review until clean.

- [ ] **Step 17: Publish Task 1 evidence only after clean review**

Create `.superpowers/sdd/2026-08-05-phase1-final-blockers-remediation/progress.md` with the Global Constraints ledger shape and Task 1 evidence. Update only the Phase 1 `TODO.md` row to state typed built-in assembly is cleanly reviewed while the editor projection cycle and final acceptance remain; keep **99%**. Do not stage or commit.

---

### Task 2: Enforce types → validators → guards → parser → service

**Files:**
- Create: `src/shared/types/dto/parameterEditorValidators.ts`
- Modify/Delete: all Task 2 files in the File Responsibility Map.
- Test: `src/features/domain/editorProjection/editorProjection.test.ts`
- Test: `src/services/nodeSystem/graphProjectionService.test.ts`
- Test: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`

**Interfaces:**
- Consumes: declarations from `editorProjection.ts`, authoritative `editor-projection.json`, existing command signatures, and existing coherence error strings.
- Produces:

```ts
// parameterEditorValidators.ts
export function isSchemaAwareParameterEditorDto(
  value: unknown,
): value is SchemaAwareParameterEditorDto;

// editorProjectionGuards.ts
export function isEditorGraphProjectionDto(
  value: unknown,
): value is EditorGraphProjectionDto;

// editorProjectionParser.ts
export function parseEditorGraphProjectionDto(
  value: unknown,
): EditorGraphProjectionDto;

export function validateEditorGraphProjection(
  projection: EditorGraphProjectionDto,
): EditorGraphProjectionDto;

// graphProjectionService.ts — unchanged public API
static loadGraph(
  graphPath: string,
  locale: string,
  lifecycleToken: number,
  projectInstanceId: string,
): Promise<EditorGraphProjectionDto>;

static hydrateGraph(
  projectInstanceId: string,
  graphPath: string,
  locale: string,
): Promise<EditorGraphProjectionDto>;
```

- Runtime dependency contract:

```text
editorProjection.ts                 (no runtime imports/exports)
parameterEditorValidators.ts        -> type-only editorProjection.ts
editorProjectionGuards.ts           -> runtime parameterEditorValidators.ts
                                    -> type-only editorProjection.ts
editorProjectionParser.ts           -> runtime editorProjectionGuards.ts
                                    -> runtime parameterEditorValidators.ts
                                    -> type-only editorProjection.ts
graphProjectionService.ts           -> runtime editorProjectionParser.ts
                                    -> type-only editorProjection.ts
feature/domain consumers            -> shared parser/types; never the reverse
```

- [ ] **Step 1: Capture Task 2 baseline without touching Task 1 evidence**

Run:

```sh
git --no-optional-locks status --short --branch
git --no-pager diff --stat
git --no-pager diff -- src/shared/types/dto src/features/domain/editorProjection src/services/nodeSystem/graphProjectionService.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
```

Expected: `shadcn`, preserved `7c49163`, Task 1 reviewed changes plus pre-existing dirty work, and no staged files.

- [ ] **Step 2: Write the RED import-graph architecture test**

In `editorProjection.test.ts`, read the five production modules and classify non-`import type` imports/re-exports. Assert:

```ts
expect(runtimeEdges).toEqual([
  ['parameterEditorValidators.ts', 'editorProjection.ts', 'type-only'],
  ['editorProjectionGuards.ts', 'parameterEditorValidators.ts', 'runtime'],
  ['editorProjectionGuards.ts', 'editorProjection.ts', 'type-only'],
  ['editorProjectionParser.ts', 'editorProjectionGuards.ts', 'runtime'],
  ['editorProjectionParser.ts', 'parameterEditorValidators.ts', 'runtime'],
  ['editorProjectionParser.ts', 'editorProjection.ts', 'type-only'],
  ['graphProjectionService.ts', 'editorProjectionParser.ts', 'runtime'],
  ['graphProjectionService.ts', 'editorProjection.ts', 'type-only'],
]);
```

Also recursively scan production `src/services/**/*.ts` for `@/features` or `@/views`, shared DTO modules for `@/features`, `@/services`, or `@/views`, and reject runtime re-exports from `editorProjection.ts`.

- [ ] **Step 3: Add RED service behavior assertions**

Extend `graphProjectionService.test.ts` so `invoke` returns `unknown`, both methods accept the authoritative fixture, both malformed root and malformed nested parameter configuration reject with `Invalid editor graph projection response`, and exact calls remain:

```ts
['load_project_graph', {
  graphPath: 'functions/main',
  locale: 'zh-CN',
  lifecycleToken: 7,
  projectInstanceId: 'project-instance-1',
}]

['hydrate_editor_graph', {
  projectInstanceId: 'project-instance-1',
  graphPath: 'functions/main',
  locale: 'en-US',
}]
```

- [ ] **Step 4: Run RED and record the cycle**

Run:

```sh
pnpm test src/features/domain/editorProjection/editorProjection.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
```

Expected: architecture failure because `editorProjection.ts` runtime re-exports `editorProjectionGuards.ts`, guards runtime-import `isSchemaAwareParameterEditorDto` from `editorProjection.ts`, `parameterEditorValidators.ts` is absent, and the feature shim still exists. Record the actual failure.

- [ ] **Step 5: Make `editorProjection.ts` declarations-only**

Move `relationalScalarTypes`, `filterOperators`, `hasExactKeys`, `isColumnOption`, `isFilterLiteral`, `isFilterPredicate`, and `isSchemaAwareParameterEditorDto` out of `editorProjection.ts`. Delete `export { isEditorGraphProjectionDto } from './editorProjectionGuards'`. Leave every interface/union property and wire spelling unchanged.

- [ ] **Step 6: Create focused parameter-editor validators**

Create `parameterEditorValidators.ts` with `import type { ... } from './editorProjection'`. Preserve exact-key rules, closed scalar/operator/literal sets, null-check predicate key rules, finite JSON number behavior, and the existing type predicate signature.

- [ ] **Step 7: Make guards structurally one-way**

Change `editorProjectionGuards.ts` to `import type` DTO declarations and runtime-import only `isSchemaAwareParameterEditorDto` from the validator module. Keep all exact-key arrays and validation predicates unchanged. Export only `isEditorGraphProjectionDto`.

- [ ] **Step 8: Make parser structurally first and coherence second**

Change `editorProjectionParser.ts` to type-only DTO imports, runtime guard import from `editorProjectionGuards`, and runtime parameter validator import from `parameterEditorValidators`. Preserve `parseEditorGraphProjectionDto`'s public sanitized error and every coherence error string/check in its current order.

- [ ] **Step 9: Delete the feature parser shim and migrate consumers directly**

Delete `features/domain/editorProjection/validateProjection.ts`; remove its export from the feature index; import `validateEditorGraphProjection` directly in `toProjectionEntities.ts` and its tests. Do not add a temporary runtime re-export. Keep `portAddressKey`, `toProjectionEntities`, and feature-owned entity types in the feature module.

- [ ] **Step 10: Update strict contract imports without weakening cases**

Import `isEditorGraphProjectionDto` directly from guards and `isSchemaAwareParameterEditorDto` directly from validators in golden/behavior tests. Retain all authoritative fixture acceptance and every unknown-key, missing-key, discriminant, fingerprint, safe-integer, finite-position, duplicate, ownership, endpoint, direction, and parameter-editor mutation.

- [ ] **Step 11: Run focused GREEN frontend suites**

Run:

```sh
pnpm test src/features/domain/editorProjection/editorProjection.test.ts src/features/core/dataStore/graphProjectionStore.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/services/nodeSystem/catalogService.test.ts
pnpm typecheck
git --no-pager diff --check
```

Expected: all tests and typecheck pass; service command names/arguments and parser errors are unchanged.

- [ ] **Step 12: Run direct dependency audits**

Run:

```sh
git --no-pager grep -n "@/features\|@/views" -- "src/services/**/*.ts"
git --no-pager grep -n "@/features\|@/services\|@/views" -- "src/shared/types/dto/editorProjection*.ts" "src/shared/types/dto/parameterEditorValidators.ts"
git --no-pager grep -n "validateProjection" -- "src/**/*.ts" "src/**/*.tsx"
```

Expected: no production service/shared violations and no obsolete `validateProjection` import/file. Test-only architecture token strings may appear only in tests. Confirm the architecture test reports no runtime cycle.

- [ ] **Step 13: Obtain a clean independent Task 2 review**

Give a fresh reviewer the approved design, original quality blocker report, Task 2 diff only, RED/GREEN output, direct dependency audits, and authoritative fixture test output. Require severity findings for exact module ownership, type-only edges, absence of runtime barrels/cycles, no service→feature edge, structural-first parsing, complete malformed matrix, exact public error, and unchanged IPC arguments. Resolve every Critical/Important finding with a focused failing test, rerun Steps 11-12, and repeat review until clean.

- [ ] **Step 14: Publish Task 2 evidence only after clean review**

Append Task 2 evidence/contracts to the new remediation ledger. Update only the Phase 1 `TODO.md` row to state both final blockers are cleanly reviewed but final whole-slice review and fresh verification remain; keep **99%**. Do not stage or commit.

---

### Task 3: Re-review the whole slice, verify fixtures, and publish Phase 1 100%

**Files:**
- Review: every path changed by Tasks 1 and 2, the original Phase 1 plan/design/ledger/final-quality report, and the remediation design/plan/ledger.
- Read-only: all five `src/tests/fixtures/node-system-contracts/*.json` files.
- Modify after every acceptance gate passes: `.superpowers/sdd/2026-08-05-phase1-final-blockers-remediation/progress.md`
- Modify after every acceptance gate passes: `TODO.md`
- Modify for checkbox tracking only: `docs/superpowers/plans/2026-08-05-phase1-final-blockers-remediation.md`

**Interfaces:**
- Consumes: two clean independently reviewed deliverables and their RED/GREEN evidence.
- Produces: clean spec review, clean quality review, identical five-fixture SHA-256 hash sets, fresh full verification, final hygiene evidence, and a Phase 1 100% publication.
- No production code is introduced. Any Critical/Important finding reopens the owning task and requires its focused RED-GREEN-review loop before this task resumes.

- [ ] **Step 1: Capture final acceptance baseline and protected boundaries**

Run:

```sh
git --no-optional-locks status --short --branch
git --no-pager log -1 --oneline --decorate
git --no-pager diff --stat
git --no-pager diff --check
```

Expected: branch `shadcn`, protected commit `7c49163` remains in history, no staged/committed remediation work, and unrelated dirty work remains intact.

- [ ] **Step 2: Record pre-test SHA-256 hashes for all five fixtures**

Run exactly:

```sh
sha256sum src/tests/fixtures/node-system-contracts/editor-projection.json src/tests/fixtures/node-system-contracts/fingerprint-wire.json src/tests/fixtures/node-system-contracts/i18n-inventory.json src/tests/fixtures/node-system-contracts/localized-catalog.json src/tests/fixtures/node-system-contracts/semantic-protocol.json
```

Record the five-line output in the remediation ledger working notes. The planning-time reference values were:

```text
ac79331ed3ac7a775cc2da2c0ccdea336521b3a33aed407c6ce1b2ffb4523ed1  editor-projection.json
8dd4214d61482a2cb44f0f03095ec511e599933a9f98b3d98b42eca36b0ec15c  fingerprint-wire.json
6323acf61d5f9e730508767766c98c3cd7f526bb2e4afd592d1bef0c86485675  i18n-inventory.json
91740cd68e6c37cc6d413695f7f1658e08c8318c317d9cdb7b6fee8ac578b1ed  localized-catalog.json
66ccb867f32aa192b56afd81c596ceac64bfc99f47cd50fc2fa088e4d9c4a0ae  semantic-protocol.json
```

The acceptance comparison is pre-test versus post-test in this execution; do not fail merely because an intentionally reviewed earlier task changed the planning-time reference.

- [ ] **Step 3: Run focused Rust acceptance serially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::source_audit -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests_dynamic -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::document::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_store::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_state::startup_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::contracts -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
```

Expected: PASS. Record exact test counts and inherited warnings separately; do not attribute pre-existing `production_tests.rs` warnings to this remediation.

- [ ] **Step 4: Run focused frontend acceptance**

Run:

```sh
pnpm test src/features/domain/editorProjection/editorProjection.test.ts src/features/core/dataStore/graphProjectionStore.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/services/nodeSystem/catalogService.test.ts
pnpm typecheck
```

Expected: PASS with authoritative fixture parsing, malformed rejection, coherence checks, and exact service IPC calls preserved.

- [ ] **Step 5: Re-run static blocker audits**

Run:

```sh
git --no-pager grep -n "ASSEMBLY_PROTOCOL_ERROR\|record_protocol_error\|run_assembly\|AssemblySemanticId\|from_unvalidated_assembly" -- "src-tauri/src/**/*.rs"
git --no-pager grep -n "@/features\|@/views" -- "src/services/**/*.ts"
git --no-pager grep -n "@/features\|@/services\|@/views" -- "src/shared/types/dto/editorProjection*.ts" "src/shared/types/dto/parameterEditorValidators.ts"
git --no-pager grep -n "validateProjection" -- "src/**/*.ts" "src/**/*.tsx"
```

Expected: no production violations; only test audit token strings are permitted.

- [ ] **Step 6: Obtain an independent whole-slice spec review**

Give a fresh reviewer both approved designs, both plans, the original ledger/final-quality report, the new ledger, complete Task 1-2 diff, focused outputs, static audits, and all five fixtures. Require an explicit PASS/CHANGES_REQUESTED verdict and severity list covering every goal, non-goal, delivery constraint, behavior-preservation contract, task order, and publication gate. Any Critical/Important finding reopens Task 1 or Task 2.

- [ ] **Step 7: Obtain a separate independent whole-slice quality review**

Use a reviewer who did not perform Step 6. Require explicit review of error source chains, fail-fast startup ordering, absence of out-of-band state/fallback/panic, test fault fidelity, frontend runtime dependency graph, parser completeness/error compatibility, fixture read-only safety, test quality, and dirty-work preservation. The verdict must contain no Critical or Important findings before continuing.

- [ ] **Step 8: Resolve findings through the owning RED-GREEN loop**

For each Critical/Important finding, return to Task 1 Step 2/3/4 or Task 2 Step 2/3, add one focused failing regression, run the relevant RED command, implement the minimal correction, rerun that task's complete GREEN matrix and independent task review, then repeat Steps 3-7. Do not repair production code directly inside Task 3.

- [ ] **Step 9: Run fresh full cross-layer verification exactly**

Only after both whole-slice reviews are clean, run exactly:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify
```

Expected: frontend typecheck/full Vitest, Rust fmt/check/full Rust and science tests, and diff check all pass. Earlier focused output is not a substitute. If it fails, classify the failure, reopen the owning task for remediation, and repeat clean review before rerunning this command.

- [ ] **Step 10: Prove ordinary tests did not rewrite fixtures**

Run the exact Step 2 `sha256sum` command again and compare all five full hashes line-by-line with the pre-test set. Then run:

```sh
git --no-pager diff -- src/tests/fixtures/node-system-contracts/editor-projection.json src/tests/fixtures/node-system-contracts/fingerprint-wire.json src/tests/fixtures/node-system-contracts/i18n-inventory.json src/tests/fixtures/node-system-contracts/localized-catalog.json src/tests/fixtures/node-system-contracts/semantic-protocol.json
```

Expected: all five pre/post hashes are identical and ordinary tests introduced no fixture diff. Never set `YSSBI_UPDATE_NODE_CONTRACT_FIXTURES` during acceptance.

- [ ] **Step 11: Run final workspace hygiene**

Run:

```sh
git --no-optional-locks status --short --branch
git --no-pager diff --check
git --no-pager diff -- docs/superpowers/plans/2026-08-05-phase1-final-blockers-remediation.md .superpowers/sdd/2026-08-05-phase1-final-blockers-remediation/progress.md TODO.md
```

Confirm `shadcn`, protected `7c49163`, no stage/commit/tag/push/worktree operation, no fixture rewrite, and all unrelated dirty files preserved.

- [ ] **Step 12: Publish Phase 1 100% only after every gate is clean**

Append to the new ledger: both whole-slice reviewer verdicts, focused command counts, exact fresh `pnpm verify` result, pre/post five-fixture hashes, static audit results, and final hygiene status. Update only the Phase 1 row in `TODO.md` to **100%**, stating that stable identity/Registry closure, typed fail-fast built-in assembly, one-way editor projection parsing, golden fixture immutability, clean whole-slice reviews, and fresh full verification are complete. Run `git --no-pager diff --check` once more. Do not stage, commit, tag, or push.

---

## Plan Self-Review Checklist

- [x] Header: exact writing-plans header, goal, architecture, stack, and global constraints are present.
- [x] Scope: exactly three tasks cover typed built-in assembly, one-way editor projection ownership, and final independent acceptance/publication.
- [x] File map: every create/modify/delete/review/evidence path has one stated responsibility; production files are changed only in Tasks 1-2.
- [x] RED-GREEN: Tasks 1 and 2 begin with concrete failing tests and named RED commands, then require focused GREEN matrices.
- [x] Independent review: every task has a clean independent gate; Task 3 requires separate whole-slice spec and quality reviewers.
- [x] Type consistency: Rust errors/helper/factory signatures and TypeScript validator/guard/parser/service signatures match across producers and consumers.
- [x] Behavior preservation: stable IDs, valid protocols, localization, Registry fingerprints, fixtures, projection validation, public error text, and IPC calls are explicitly frozen.
- [x] Delivery safety: direct `shadcn`, protected `7c49163`, dirty-work preservation, no worktree/stage/commit/tag/push/reset/revert, and no commit steps are explicit.
- [x] Publication discipline: the new ledger and Phase 1 `TODO.md` row update only after clean reviews; 100% requires clean whole-slice reviews, identical fixture hashes, fresh exact `pnpm verify`, and final hygiene.
- [x] Placeholder and ambiguity scan: no unspecified implementation choice, deferred decision, placeholder section, or unnamed command remains.
