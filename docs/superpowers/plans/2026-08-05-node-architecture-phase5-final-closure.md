# Node Architecture Phase 5 Final Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace opaque English compiler diagnostics with one typed, complete, localizable definition authority and prove deterministic non-empty diagnostic snapshots.

**Architecture:** The generic `analysis::NodeDiagnostic` wire model remains unchanged. A new compiler-owned declarative definition module generates typed diagnostic payloads, stable definitions, templates, validation, conversion, and canonical comparison. Built-in localization consumes the generated definitions; compiler passes emit typed semantic facts; projection remains the sole message-rendering boundary.

**Tech Stack:** Rust, serde, existing node-system compiler/catalog/projection modules, syn source audits, pnpm Cargo scripts.

## Global Constraints

- Work directly on `shadcn` and preserve unrelated dirty work.
- Do not create a worktree, branch, stage, commit, amend, tag, push, reset, revert, restore, or clean.
- Use RED-GREEN for every behavior change.
- Run focused Rust tests with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
- Do not change frontend diagnostic DTOs or rendering APIs.
- Do not modify runtime/scientific diagnostics.
- Do not migrate provider-facing `LoweringError`, `InterfaceResolverError`, or `SchemaResolutionError`; map them to stable facts at the compiler boundary.
- Do not serialize provider error prose into `AnalysisSnapshot`; retain it only for tracing/logging when needed.
- One declaration must own code, message key, severity, argument names, and locale templates.
- Update Phase 5 in `TODO.md` only after focused verification and independent review are clean.

---

## File map

- Create `src-tauri/src/node_system/compiler/diagnostics.rs`: typed compiler diagnostic authority, generated definitions/templates, validation, conversion, and canonical comparator.
- Modify `src-tauri/src/node_system/compiler/mod.rs`: register the private diagnostics module.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: typed emission and canonical sorting.
- Modify `src-tauri/src/node_system/compiler/control.rs`: typed `ControlIssue` payloads.
- Modify `src-tauri/src/node_system/compiler/dynamic_interface.rs`: remove duplicate generic constructor.
- Modify `src-tauri/src/node_system/compiler/type_analysis.rs`: typed type-analysis issues.
- Modify `src-tauri/src/node_system/compiler/schema_analysis.rs`: typed schema-analysis issues.
- Modify `src-tauri/src/node_system/catalog/builtin.rs`: consume generated requirements/templates and validate definitions.
- Modify `src-tauri/src/node_system/catalog/tests.rs`: completeness, startup rejection, and locale rendering tests.
- Modify `src-tauri/src/node_system/compiler/tests.rs`: non-empty diagnostic insertion-order differential test.
- Modify `src-tauri/src/node_system/testing/source_audit.rs`: enforce typed emission authority.
- Update only if explicitly generated and reviewed: `src/tests/fixtures/node-system-contracts/i18n-inventory.json`.
- Modify after clean review: `TODO.md`.
- Create `.superpowers/sdd/2026-08-05-node-architecture-phase5-final-closure/progress.md`.

---

### Task 1: Establish failing authority and completeness contracts

**Files:**
- Modify: `src-tauri/src/node_system/testing/source_audit.rs`
- Modify: `src-tauri/src/node_system/catalog/tests.rs`

**Interfaces:**
- Produces acceptance constraints for the later `CompilerDiagnostic` and `COMPILER_DIAGNOSTIC_DEFINITIONS` APIs.

- [ ] **Step 1: Add the typed-emission source-audit RED test**

Add `production_compiler_diagnostics_use_only_typed_definition_authority`. Scan non-test Rust syntax under `node_system/compiler`, excluding the future `compiler/diagnostics.rs`, and reject:

- string literals beginning with `compiler.`;
- direct `NodeDiagnostic { ... }` construction;
- compiler issue fields named `code` or `detail`;
- generic diagnostic constructors accepting `code: &'static str`;
- compiler-created argument maps containing the key `"detail"`.

The audit must ignore test modules and assertions so test fixture codes do not become false positives.

- [ ] **Step 2: Run the source-audit RED test**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::testing::source_audit::production_compiler_diagnostics_use_only_typed_definition_authority --exact --test-threads=1
```

Expected: FAIL on existing generic constructors and issue structures.

- [ ] **Step 3: Add the built-in inventory completeness RED test**

Add `every_production_compiler_definition_is_required_by_builtin_i18n`. It will iterate the generated definitions and assert every `message_key` is present in provider i18n requirements. Lock the current production scope at 106 definitions while migration is in progress; after typed migration removes obsolete/non-emitted entries, this count must equal the generated enum’s exact definition count rather than a separate hand-maintained code list.

- [ ] **Step 4: Add the missing-default-template RED test**

Add `builtin_startup_rejects_missing_compiler_default_template`. Use `diagnostics.compiler.node.scope_mismatch`, remove its `en-US` message in a test bundle, and assert `I18nBundleValidationError::MissingDefaultLocale` contains that exact stable key.

- [ ] **Step 5: Run both catalog RED tests**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::catalog::tests::every_production_compiler_definition_is_required_by_builtin_i18n --exact --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::catalog::tests::builtin_startup_rejects_missing_compiler_default_template --exact --test-threads=1
```

Expected: FAIL because current requirements derive from the stale 82-code list and omit `scope_mismatch`.

- [ ] **Step 6: Obtain independent Task 1 review**

Reviewer must confirm the audit is syntax-aware, excludes tests without excluding production nested modules, and cannot be satisfied by moving untyped construction to another compiler file.

---

### Task 2: Implement the single typed diagnostic definition authority

**Files:**
- Create: `src-tauri/src/node_system/compiler/diagnostics.rs`
- Modify: `src-tauri/src/node_system/compiler/mod.rs`
- Test: `src-tauri/src/node_system/compiler/diagnostics.rs`

**Interfaces:**
- Produces:

```rust
pub(crate) type CompilerDiagnosticLocation =
    DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>;

pub(crate) type CompilerNodeDiagnostic =
    NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticTemplate {
    pub locale: &'static str,
    pub text: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompilerDiagnosticDefinition {
    pub code: &'static str,
    pub message_key: &'static str,
    pub default_severity: DiagnosticSeverity,
    pub argument_names: &'static [&'static str],
    pub templates: &'static [DiagnosticTemplate],
}
```

- Produces generated `CompilerDiagnostic`, `COMPILER_DIAGNOSTIC_DEFINITIONS`, definition validation, conversion, and comparison.

- [ ] **Step 1: Add definition-validation unit tests**

Add:

```rust
#[test]
fn compiler_diagnostic_definitions_are_unique_and_template_safe()
```

It must assert unique codes and keys, an `en-US` template for every definition, and exact equality between declared placeholders and placeholders referenced by each locale template.

Add:

```rust
#[test]
fn compiler_diagnostic_constructor_emits_only_declared_arguments()
```

Construct at least `NodeUnknown { node_type }`, `InputUnbound {}`, and `TypeIncompatible { expected_type, actual_type }`; assert exact code, key, Error severity, named argument maps, and absence of `detail`.

- [ ] **Step 2: Verify tests fail because the module/API does not exist**

Run both exact tests after registering the empty module. Expected: compile failure or test failure specifically due to missing generated definitions.

- [ ] **Step 3: Implement the declarative macro and definition model**

Create one macro invocation that declares every production compiler diagnostic. Each entry binds variant fields directly to argument names and provides exact `en-US` and `zh-CN` templates. The macro generates:

```rust
pub(crate) enum CompilerDiagnostic { ... }

pub(crate) const COMPILER_DIAGNOSTIC_DEFINITIONS:
    &[CompilerDiagnosticDefinition];

impl CompilerDiagnostic {
    pub(crate) fn definition(&self)
        -> &'static CompilerDiagnosticDefinition;

    pub(crate) fn into_node(
        self,
        primary: CompilerDiagnosticLocation,
    ) -> CompilerNodeDiagnostic;

    pub(crate) fn into_node_with_related(
        self,
        primary: CompilerDiagnosticLocation,
        related: impl Into<Box<[CompilerDiagnosticLocation]>>,
    ) -> CompilerNodeDiagnostic;
}
```

Use snake_case semantic arguments such as `node_type`, `port`, `expected_type`, `actual_type`, `expected_scope`, `actual_scope`, `function_path`, `resolver_id`, `parameter_key`, `field_name`, `source_name`, and `target_name`. Templates reference only declared names.

- [ ] **Step 4: Implement typed definition errors**

Add:

```rust
pub(crate) enum CompilerDiagnosticDefinitionError {
    DuplicateCode { code: Box<str> },
    DuplicateMessageKey { message_key: Box<str> },
    MissingDefaultTemplate {
        code: Box<str>,
        message_key: Box<str>,
    },
    ArgumentTemplateMismatch {
        code: Box<str>,
        locale: Box<str>,
        declared: Vec<Box<str>>,
        referenced: Vec<Box<str>>,
    },
}
```

Implement `Display` and `Error`, then:

```rust
pub(crate) fn validate_compiler_diagnostic_definitions(
    definitions: &[CompilerDiagnosticDefinition],
) -> Result<(), CompilerDiagnosticDefinitionError>;
```

Placeholder extraction must recognize the catalog’s existing `{name}` interpolation syntax and compare canonical sorted sets.

- [ ] **Step 5: Implement canonical comparison**

Add `compare_diagnostics(left, right)` ordering by canonical primary location, code, arguments, and canonical related locations. Never use localized text or template order.

- [ ] **Step 6: Run Task 2 unit tests GREEN**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::diagnostics::tests::compiler_diagnostic_definitions_are_unique_and_template_safe --exact --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::diagnostics::tests::compiler_diagnostic_constructor_emits_only_declared_arguments --exact --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Obtain independent Task 2 review**

Reviewer must verify the macro invocation is the sole authority, no synchronized code array exists, conversion emits only declared arguments, and all definition failures preserve typed sources.

---

### Task 3: Integrate typed definitions with built-in localization

**Files:**
- Modify: `src-tauri/src/node_system/catalog/builtin.rs`
- Modify: `src-tauri/src/node_system/catalog/tests.rs`

**Interfaces:**
- Consumes: `COMPILER_DIAGNOSTIC_DEFINITIONS` and `validate_compiler_diagnostic_definitions`.
- Produces: complete required i18n inventory and exact per-diagnostic templates.

- [ ] **Step 1: Add typed assembly error propagation**

Add `BuiltinAssemblyError::DiagnosticDefinitions { source: CompilerDiagnosticDefinitionError }`. Implement `Display` and return `Some(source)` from `Error::source()`.

- [ ] **Step 2: Validate definitions before returning built-in parts**

Call `validate_compiler_diagnostic_definitions(COMPILER_DIAGNOSTIC_DEFINITIONS)` in the typed built-in assembly chain before Registry/catalog completion. Map it without string flattening.

- [ ] **Step 3: Replace the stale code list**

Delete `COMPILER_DIAGNOSTIC_CODES`. Build required keys by iterating each definition’s `message_key`. Build localization messages by iterating every definition/template pair. Delete generic `Compiler diagnostic: {detail}` and `编译诊断：{detail}` templates.

- [ ] **Step 4: Run Task 1 catalog tests GREEN**

Run the two exact catalog commands from Task 1. Expected: PASS, with requirements derived only from definitions.

- [ ] **Step 5: Run localization and assembly tests**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::catalog::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::catalog::builtin::tests --test-threads=1
```

Expected: PASS after updating obsolete generic-message assertions to exact templates.

- [ ] **Step 6: Obtain independent Task 3 review**

Reviewer must verify every emitted definition becomes a required default-locale key, unsupported locales retain documented fallback, and Registry/protocol fingerprints are not coupled to localized text.

---

### Task 4: Migrate all production compiler emissions

**Files:**
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/control.rs`
- Modify: `src-tauri/src/node_system/compiler/dynamic_interface.rs`
- Modify: `src-tauri/src/node_system/compiler/type_analysis.rs`
- Modify: `src-tauri/src/node_system/compiler/schema_analysis.rs`

**Interfaces:**
- Consumes: generated `CompilerDiagnostic` and `compare_diagnostics`.
- Produces: locale-independent named semantic arguments in every production compiler diagnostic.

- [ ] **Step 1: Change the central push API**

Replace string-based push/construction with:

```rust
fn push(
    &mut self,
    diagnostic: CompilerDiagnostic,
    location: CompilerDiagnosticLocation,
) {
    self.diagnostics.push(diagnostic.into_node(location));
}
```

Delete the generic `diagnostic(code, primary, detail)` helper.

- [ ] **Step 2: Migrate pipeline emissions**

Replace every production `"compiler.*"` code plus formatted detail with its generated variant and named facts. For errors originating outside the compiler, include stable facts such as node type, port key/address, resolver ID, or resource identity; do not serialize `error.to_string()`.

- [ ] **Step 3: Migrate intermediate issue structures**

Use:

```rust
pub(crate) struct ControlIssue {
    pub node_id: Option<NodeId>,
    pub diagnostic: CompilerDiagnostic,
}

pub(crate) struct TypeAnalysisIssue {
    pub location: CompilerDiagnosticLocation,
    pub diagnostic: CompilerDiagnostic,
}

pub(crate) struct SchemaAnalysisIssue {
    pub location: CompilerDiagnosticLocation,
    pub diagnostic: CompilerDiagnostic,
}
```

Delete their `code` and `detail` fields. Type mismatches must carry `expected_type` and `actual_type`. Schema issues must carry stable resolver/parameter/field/source/target names as appropriate.

- [ ] **Step 4: Remove dynamic-interface duplicate construction**

Delete its private generic diagnostic helper and construct generated variants directly.

- [ ] **Step 5: Use canonical comparison everywhere**

Replace both main-analysis sorting and append-time sorting with `compare_diagnostics`. Ensure diagnostics sharing primary location and code are ordered by semantic arguments and related locations.

- [ ] **Step 6: Run the typed-emission source audit GREEN**

Run Task 1 Step 2. Expected: PASS with no production compiler code strings or `detail` escape hatch outside the authority module.

- [ ] **Step 7: Run focused compiler suites**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::tests_dynamic --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::tests_dynamic_pipeline --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::schema_analysis::tests --test-threads=1
```

Expected: all PASS after replacing assertions on English `detail` with exact named facts.

- [ ] **Step 8: Obtain independent Task 4 review**

Reviewer must sample every compiler pass, verify all 106 production codes are represented exactly once, reject English prose/provider error strings in snapshot arguments, and confirm test-only synthetic diagnostics are unaffected.

---

### Task 5: Prove locale invariance at the projection boundary

**Files:**
- Modify: `src-tauri/src/node_system/catalog/tests.rs`

**Interfaces:**
- Consumes: one immutable invalid `AnalysisSnapshot` and existing projection localization.
- Produces: proof that locale changes only `DiagnosticDto.message`.

- [ ] **Step 1: Add the locale-invariance test**

Add `diagnostic_projection_changes_only_localized_message`. Compile a fixed invalid graph once. Serialize its `AnalysisSnapshot`. Build `en-US` and `zh-CN` projections from the same snapshot. Assert:

- snapshot bytes are unchanged before and after projection;
- projected code, severity, blocking flag, location, and related locations are identical;
- messages differ and equal exact locale templates;
- snapshot JSON contains neither rendered message;
- snapshot arguments contain no `detail` key.

- [ ] **Step 2: Run the locale test**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::catalog::tests::diagnostic_projection_changes_only_localized_message --exact --test-threads=1
```

Expected: PASS after typed emission/catalog integration.

- [ ] **Step 3: Obtain independent Task 5 review**

Reviewer must confirm the same immutable snapshot is used for both locales and assertions do not merely compare translated prefixes around English prose.

---

### Task 6: Prove deterministic non-empty diagnostics

**Files:**
- Modify: `src-tauri/src/node_system/compiler/tests.rs`

**Interfaces:**
- Consumes: canonical diagnostic comparator and fixed compiler fixtures.
- Produces: insertion-order/seed-independent invalid `AnalysisSnapshot` bytes.

- [ ] **Step 1: Add seeded fixture insertion order**

Extend the local fixture order enum with `Seeded(u64)`. Shuffle insertion collections with `StdRng::seed_from_u64` and `SliceRandom`. Use seeds `0`, `1`, and `0x5eed_5eed_5eed_5eed`.

- [ ] **Step 2: Add the invalid differential test**

Add `invalid_analysis_snapshot_is_deterministic_across_insertion_orders`. Use fixed graph path, revision, registry fingerprint, compile ID, resource versions, node UUIDs, connection UUIDs, dynamic `PortInstanceId`, and `OrderKey`.

Produce at least three diagnostics from distinct domains, including unknown node, required unbound input, and incompatible concrete types. Assert diagnostics are non-empty, semantic graph and plan are absent, and forward/reverse/seeded builds have byte-identical serialized analyses and exact diagnostic vectors. Assert exact equality of code, arguments, severity, primary location, and related locations. Snapshot JSON must contain no `message` field and no `detail` argument.

- [ ] **Step 3: Run the invalid differential test**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::tests::invalid_analysis_snapshot_is_deterministic_across_insertion_orders --exact --test-threads=1
```

Expected: PASS after canonical comparison. If RED reveals equal-code/location nondeterminism, fix only the comparator or originating unordered collection.

- [ ] **Step 4: Preserve valid-graph determinism**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::tests::semantically_identical_documents_serialize_identically --exact --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Obtain independent Task 6 review**

Reviewer must confirm the fixture actually emits multiple diagnostics, uses fixed identities, varies insertion order rather than semantic content, and does not require invalid graphs to produce semantic/plan artifacts.

---

### Task 7: Verify contracts and publish Phase 5

**Files:**
- Update only if required after explicit review: `src/tests/fixtures/node-system-contracts/i18n-inventory.json`
- Create: `.superpowers/sdd/2026-08-05-node-architecture-phase5-final-closure/progress.md`
- Modify after clean review only: `TODO.md`

- [ ] **Step 1: Run the focused Phase 5 matrix**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::compiler::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::catalog::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::registry::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::analysis::projection::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::testing::contracts::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
```

```sh
pnpm rust:fmt:check
```

```sh
git diff --check
```

Record exact counts and warnings.

- [ ] **Step 2: Review any i18n fixture change explicitly**

If the generated i18n inventory differs, prove the diff consists only of the exact generated compiler message-key set and expected fingerprint fields. Do not update semantic protocol, editor projection, localized catalog, or fingerprint-wire fixtures unless their production contract genuinely changed and receives separate review.

- [ ] **Step 3: Obtain whole-slice Phase 5 review**

Request independent spec and quality reviews. Resolve every Critical/Important finding with focused RED-GREEN and repeat affected verification.

- [ ] **Step 4: Create the Phase 5 ledger**

Record each task/fix round, generated definition count, exact focused commands/counts, fixture hashes/diffs, reviews, branch/HEAD/staging state, and dirty-work preservation.

- [ ] **Step 5: Publish Phase 5 at 100%**

Only after clean review, update only the Phase 5 row in `TODO.md`. State that typed diagnostics, complete localization inventory, locale-independent snapshots, canonical non-empty diagnostic ordering, and fresh focused verification are complete.

- [ ] **Step 6: Run final architecture acceptance**

After every TODO phase is 100%:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify
```

Recompute protected node-system fixture SHA-256 values, inspect branch/HEAD/staging state, and run:

```sh
git --no-pager diff --check
```

Append exact evidence to the Phase 5 ledger. Do not claim complete until the full verify exits 0.
