# DataFrame Decompose Schema Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `yssbi.dataframe.decompose` derive stable DataSeries output ports from the authoritative upstream DataFrame schema without emitting `compiler.interface.resolver_missing`.

**Architecture:** Add stable optional lineage to schema fields, then split compiler interface resolution into a provisional pass and a schema-dependent completion pass. A focused built-in `DataframeColumnsResolver` consumes preliminary schema facts and maps fields to dynamic members; the existing `SchemaAnalyzer` remains the sole schema propagation implementation and the final pass remains authoritative.

**Tech Stack:** Rust, serde, the node-system compiler and protocol, `SchemaAnalyzer`, dynamic interface materialization, project resource snapshots, Cargo tests through repository-root `pnpm` scripts.

## Global Constraints

- Work in the existing workspace and preserve unrelated dirty changes.
- Do not create a branch, worktree, commit, or tag unless the user explicitly requests it.
- `SchemaAnalyzer` remains the only implementation of schema propagation and DataFrame transformation semantics.
- `ProjectState.project_data` remains authoritative; do not add frontend-owned schema or graph state.
- `ProjectState::insert_graph` and graph lifecycle behavior are out of scope.
- Do not hold project/global locks while reading database schemas or compiling.
- Provider resolver declarations and production resolver registrations must stay consistent.
- `InterfaceResolverMissing` is emitted only when no implementation is registered for the referenced ID.
- Missing/invalid upstream schema must not be converted into a successful empty-column result.
- No frontend contract or runtime DataFrame representation changes.
- Add focused regression tests before each behavior change.
- Run Rust commands from the repository root through `pnpm`; do not invoke ad-hoc Cargo commands that create `src-tauri/target/`.
- Run focused tests, then `pnpm rust:check` and `git diff --check` before completion.

## File Structure

- Modify `src-tauri/src/node_system/protocol/types.rs`: add optional compiler-owned schema field lineage while keeping display name and scalar type unchanged.
- Modify existing `SchemaField` construction sites in `src-tauri/src/node_system/analysis/`, `compiler/`, and focused tests: explicitly use `lineage: None` where no authoritative lineage exists.
- Modify `src-tauri/src/node_system/compiler/schema_analysis.rs`: preserve lineage through Filter, Project, Rename, and existing schema transformations.
- Modify `src-tauri/src/project/project_state.rs`: attach canonical database/column lineage in `ProjectDatabaseSchemaResolver`.
- Modify `src-tauri/src/node_system/compiler/dynamic_interface.rs`: expose preliminary schema facts to interface resolvers and represent schema-dependent deferral without a false missing-resolver diagnostic.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: perform provisional interface resolution, preliminary schema analysis, schema-dependent interface completion, then existing authoritative validation/analysis.
- Create `src-tauri/src/node_system/compiler/dataframe.rs`: own `DataframeColumnsResolver` and the complete built-in resolver assembly entry point.
- Modify `src-tauri/src/node_system/compiler/project.rs` and `compiler/mod.rs`: keep function resolver implementation focused and export the complete built-in resolver builder.
- Modify `src-tauri/src/node_system/catalog/dataframe/tests.rs`: test built-in protocol/resolver consistency and transformed-schema Decompose behavior.
- Modify `src-tauri/src/node_system/compiler/tests_dynamic_pipeline.rs`: test generic staged schema-dependent interface behavior independent of DataFrame catalog details.
- Modify `src-tauri/src/project/production_tests.rs`: verify the production database snapshot and resolver construction path.

---

### Task 1: Add Stable Schema Field Lineage

**Files:**
- Modify: `src-tauri/src/node_system/protocol/types.rs:67-125`
- Modify: `src-tauri/src/node_system/compiler/schema_analysis.rs:206-330,415-590`
- Modify: `src-tauri/src/project/project_state.rs:7966-8018`
- Test: `src-tauri/src/node_system/compiler/schema_analysis.rs` test module
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: canonical database resource paths in the form `databases/{database-id}` and existing `SchemaColumnRef` names.
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaFieldLineage {
    pub source: Box<str>,
    pub field: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: SchemaColumnRef,
    pub scalar_type: RelationalScalarType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<SchemaFieldLineage>,
}
```

Later tasks rely on `SchemaField.lineage` to create `DynamicMemberLocator::SchemaField` and choose `SchemaFieldIdentityGuarantee`.

- [ ] **Step 1: Write failing protocol serialization tests for optional lineage**

Add tests beside the protocol type tests proving legacy facts omit lineage and stable facts round-trip it:

```rust
#[test]
fn schema_field_lineage_is_optional_and_round_trips() {
    let legacy = SchemaField {
        name: SchemaColumnRef("amount".into()),
        scalar_type: RelationalScalarType::Float64,
        lineage: None,
    };
    assert_eq!(
        serde_json::to_value(&legacy).unwrap(),
        serde_json::json!({"name": "amount", "scalar_type": "Float64"})
    );

    let stable = SchemaField {
        name: SchemaColumnRef("amount".into()),
        scalar_type: RelationalScalarType::Float64,
        lineage: Some(SchemaFieldLineage {
            source: "databases/main".into(),
            field: "amount".into(),
        }),
    };
    assert_eq!(
        serde_json::from_value::<SchemaField>(serde_json::to_value(&stable).unwrap()).unwrap(),
        stable
    );
}
```

Update existing `SchemaField` literals in the touched test module`lineage: None`; do not change their expected names or scalar types.

- [ ] **Step 2: Run the protocol-focused test and confirm RED**

Run:

```text
pnpm rust:test schema_field_lineage_is_optional_and_round_trips
```

Expected: compilation fails because `SchemaFieldLineage` and `SchemaField.lineage` do not exist.

- [ ] **Step 3: Implement the minimal protocol lineage type**

Add `SchemaFieldLineage` and the optional field exactly as shown in **Interfaces**. Update `impl From<SchemaColumnRef> for SchemaField` so inferred fields remain lineage-free:

```rust
Self {
    name,
    scalar_type: RelationalScalarType::Unknown,
    lineage: None,
}
```

Update all compile errors from existing struct literals by setting `lineage: None`; do not infer fake lineage from display names.

- [ ] **Step 4: Run the protocol test and confirm GREEN**

Run the same focused command. Expected: PASS.

- [ ] **Step 5: Write failing schema transformation tests**

Add a helper and test in `schema_analysis.rs`:

```rust
fn stable_field(name: &str) -> SchemaField {
    SchemaField {
        name: SchemaColumnRef(name.into()),
        scalar_type: RelationalScalarType::String,
        lineage: Some(SchemaFieldLineage {
            source: "databases/main".into(),
            field: name.into(),
        }),
    }
}

#[test]
fn project_filter_and_rename_preserve_field_lineage() {
    // Feed [customer_id, region] into the existing project/filter/rename helpers.
    // Assert project keeps only customer_ididentical lineage.
    // Assert filter keeps both lineage values unchanged.
    // Assert rename changes customer_id -> account_id but retains
    // source=databases/main and field=customer_id.
}
```

Use the existing `SchemaAnalyzer::project`, `filter`, and `rename` test patterns in the same module rather than constructing a second schema engine.

- [ ] **Step 6: Run the transformation test and confirm RED**

Run:

```text
pnpm rust:test project_filter_and_rename_preserve_field_lineage
```

Expected: FAIL until each transformation carries the full `SchemaField`, including lineage, instead of reconstructing name-only fields.

- [ ] **Step 7: Preserve lineage through existing transformations**

Keep these exact semantics:

```rust
// Filter: move input.fields unchanged.
// Project: select and clone the matching complete SchemaField.
// Rename: mutate only field.name; retain scalar_type and lineage.
// Append: preserve the complete fields from the existing authoritative input
// according to current Append semantics; do not synthesize identities.
```

Do not alter existing missing-column, duplicate-column, rename-conflict, predicate, or scalar-type diagnostics.

- [ ] **Step 8: Attach canonical lineage in the database schema resolver**

Change the `ProjectDatabaseSchemaResolver` field mapping to:

```rust
.map(|column| crate::node_system::protocol::SchemaField {
    name: crate::node_system::protocol::SchemaColumnRef(column.name.clone().into()),
    scalar_type: crate::node_system::protocol::RelationalScalarType::from_database_dtype(
        &column.dtype,
    ),
    lineage: Some(crate::node_system::protocol::SchemaFieldLineage {
        source: resource.into(),
        field: column.name.clone().into(),
    }),
})
```

Here `resource` is the already-validated canonical `databases/{id}` string. The stable field identity is the original database column name; Rename changes only the display name.

- [ ] **Step 9: Add and run a production lineage test**

Construct `CompileResourceSnapshot` through the existing `compile_resources_from_data` helper, resolve a database schema using `resources.schema_resolvers()`, and assert the resulting field has:

```rust
SchemaFieldLineage {
    source: "databases/main".into(),
    field: "value".into(),
}
```

Run:

```text
pnpm rust:test database_schema_resolver_attaches_canonical_field_lineage
```

Expected: PASS after the resolver change.

- [ ] **Step 10: Run the broader schema-analysis slice**

Run:

```text
pnpm rust:test node_system::compiler::schema_analysis::tests
```

Expected: PASSno changed schema diagnostics.

---

### Task 2: Stage Schema-Dependent Dynamic Interface Resolution

**Files:**
- Modify: `src-tauri/src/node_system/compiler/dynamic_interface.rs:23-115,206-298`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs:1300-1428,1865-1905,2237-2268`
- Modify: `src-tauri/src/node_system/compiler/project.rs:63-87`
- Test: `src-tauri/src/node_system/compiler/tests_dynamic.rs`
- Test: `src-tauri/src/node_system/compiler/tests_dynamic_pipeline.rs`

**Interfaces:**
- Consumes: `BTreeMap<PortAddress, ResolvedSchemaFact>` from a preliminary run of the existing `SchemaAnalyzer`.
- Produces:

```rust
pub struct InterfaceResolverRequest<'a> {
    // existing fields remain
    pub resolved_schemas: &'a BTreeMap<PortAddress, ResolvedSchemaFact>,
}

pub trait InterfaceResolver: Send + Sync {
    fn schema_dependencies(&self) -> &[PortKey] {
        &[]
    }

    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError>;
}

pub struct DynamicInterfaceResolution {
    // existing fields remain
    pub deferred_for_schema: bool,
}
```

`schema_dependencies()` is an explicit capability declaration. Empty dependencies preserve current function resolver behavior. Missing schema dependencies defer materialization without emitting `InterfaceResolverMissing` or `InterfaceResolverFailed`.

- [ ] **Step 1: Write a failing generic staged-resolution test**

In `tests_dynamic_pipeline.rs`, add a source protocola declared schema-bearing output, a consumer protocoldeclared input `dataframe` plus derived output `columns`, and a resolver:

```rust
struct SchemaDependentResolver;

impl InterfaceResolver for SchemaDependentResolver {
    fn schema_dependencies(&self) -> &[PortKey] {
        static DEPENDENCIES: std::sync::OnceLock<Box<[PortKey]>> = std::sync::OnceLock::new();
        DEPENDENCIES.get_or_init(|| vec![PortKey::new("dataframe").unwrap()].into_boxed_slice())
    }

    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError> {
        let address = PortAddress::declared(request.node_id, PortKey::new("dataframe").unwrap());
        let schema = request
            .resolved_schemas
            .get(&address)
            .ok_or_else(|| InterfaceResolverError::new("staged schema dependency was not supplied"))?;
        Ok(schema
            .fields
            .iter()
            .map(|field| InterfaceResolverMember {
                basis: request.basis.clone(),
                locator: locator(field.name.0.as_ref()),
                label: field.name.0.to_string(),
                identity: SchemaFieldIdentityGuarantee::SnapshotScoped,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }
}
```

The test must connect source output to consumer input, compile through `GraphCompiler::with_resolvers`, and assert:

```rust
assert!(!codes.contains(&"compiler.interface.resolver_missing"));
assert!(!codes.contains(&"compiler.interface.resolver_failed"));
assert_eq!(derived_output_labels(&result), vec!["amount"]);
```

- [ ] **Step 2: Run the staged-resolution test and confirm RED**

Run:

```text
pnpm rust:test schema_dependent_interface_resolves_after_preliminary_schema_analysis
```

Expected: compilation failure because request schemas, dependency declaration, and staged resolution do not exist.

- [ ] **Step 3: Add schema-aware request and explicit deferral**

In `dynamic_interface.rs`:

1. Add `resolved_schemas` to `InterfaceResolverRequest`.
2. Add default `schema_dependencies()` to the trait.
3. Before invoking a registered implementation, map dependency keys to declared addresses and check every address exists in `resolved_schemas`.
4. When any dependency is absent, call `state.add_existing_instances(spec, None)`, set `deferred_for_schema = true`, and do not push an interface diagnostic.
5. Keep the existing missing-ID branch unchanged so a genuinely unregistered ID still emits `InterfaceResolverMissing`.
6. Pass `resolved_schemas` into `resolve()` when dependencies are present.

Use an empty schema map in the public standalone `materialize_dynamic_interface()` helper so existing non-schema resolver tests retain their behavior.

- [ ] **Step 4: Add a preliminary schema helper without publishing its facts**

Refactor the existing schema setup into a side-effect-free helper on `AnalysisState`:

```rust
fn resolve_schema_facts(
    &self,
    resolvers: &SchemaResolverSet,
    resources: &mut dyn AnalysisResourceResolver,
) -> (
    BTreeMap<PortAddress, SchemaExpr>,
    BTreeMap<PortAddress, ResolvedSchemaFact>,
    Vec<SchemaAnalysisIssue>,
)
```

It must instantiate the existing `SchemaAnalyzer`, add current nodes/current ports/current data connections, and call `analyze_with_resources`. `analyze_schemas()` then calls this helper and publishes the returned expressions, facts, and diagnostics exactly once during the final authoritative pass.

- [ ] **Step 5: Implement provisional and completion passes in `AnalysisState::analyze`**

Keep the existing node registration/parameter normalization loop, but resolve ports firstan empty schema map and collect node IDs whose `DynamicInterfaceResolution.deferred_for_schema` is true.

After all nodes are present:

```rust
let (_, preliminary_schemas, _) =
    self.resolve_schema_facts(schema_resolvers, resources);
self.complete_schema_dependent_interfaces(
    &deferred_nodes,
    &preliminary_schemas,
    resources,
    interface_resolvers,
);
```

`complete_schema_dependent_interfaces` must re-run `resolve_ports` only for deferred nodes, replace those nodes' provisional port maps, replace their interface projection entries, and clear/rebuild projection-only addresses owned by those nodes. It must not mutate `GraphDocument` or persist candidates.

Do not publish preliminary schema issues. Existing final `validate_connections`, `validate_input_bindings`, `analyze_types`, and `analyze_schemas` remain authoritative and run after completion.

- [ ] **Step 6: Preserve resolver error semantics when schema never resolves**

Add tests for:

```rust
// Registered schema-dependent resolver + unbound input:
// contains compiler.port.input_unbound (or existing exact unbound code)
// does not contain resolver_missing or resolver_failed.

// Unregistered resolver:
// still contains compiler.interface.resolver_missing.
```

A missing schema is not a successful empty result. Existing persisted instances remain provisional/orphaned according to the final dynamic binding rules, while the underlying schema/resource/input diagnostic blocks lowering.

- [ ] **Step 7: Prove non-schema resolvers are not behaviorally changed**

Run existing dynamic and function-interface tests:

```text
pnpm rust:test node_system::compiler::tests_dynamic
pnpm rust:test node_system::compiler::tests_dynamic_pipeline
pnpm rust:test builtin_function_resolver_projects_function_document_members
```

Expected: PASS. Function resolvers use the default empty dependency list and resolve during the provisional pass.

- [ ] **Step 8: Prove tracked resource reads remain set-based**

Add an assertion to the staged resource test that repeated preliminary/final database reads produce one `databases/main` entry in `analysis.basis.resource_versions` and no duplicate observation. Run the focused test and expect PASS.

---

### Task 3: Implement and Register the DataFrame Columns Resolver

**Files:**
- Create: `src-tauri/src/node_system/compiler/dataframe.rs`
- Modify: `src-tauri/src/node_system/compiler/project.rs:11-55`
- Modify: `src-tauri/src/node_system/compiler/mod.rs:1-55`
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs:38-64`
- Modify: `src-tauri/src/node_system/catalog/dataframe/tests.rs`
- Test: `src-tauri/src/node_system/catalog/dataframe/tests.rs`

**Interfaces:**
- Consumes: `SchemaField { name, scalar_type, lineage }` at declared input `dataframe`.
- Produces:

```rust
pub(crate) struct DataframeColumnsResolver;

pub fn build_builtin_interface_resolvers() -> InterfaceResolverSet;
```

The complete builder installs all four function interface resolvers plus `DATAFRAME_COLUMNS_RESOLVER`. The public builder name remains unchanged so production call sites automatically receive the complete set.

- [ ] **Step 1: Write a failing resolver-registration test**

Add to DataFrame catalog tests:

```rust
#[test]
fn dataframe_columns_resolver_is_installed_in_builtin_set() {
    let resolvers = crate::node_system::compiler::build_builtin_interface_resolvers();
    let id = InterfaceResolverId::new(DATAFRAME_COLUMNS_RESOLVER).unwrap();
    assert!(resolvers.get(&id).is_some());
}
```

Also assert the Decompose protocol's `columns` template references exactly the same ID.

- [ ] **Step 2: Run the registration test and confirm RED**

Run:

```text
pnpm rust:test dataframe_columns_resolver_is_installed_in_builtin_set
```

Expected: FAIL because the ID is advertised but has no implementation.

- [ ] **Step 3: Create the focused DataFrame resolver module**

Implement `src-tauri/src/node_system/compiler/dataframe.rs`this behavior:

```rust
const DATAFRAME_INPUT: &str = "dataframe";

impl InterfaceResolver for DataframeColumnsResolver {
    fn schema_dependencies(&self) -> &[PortKey] {
        static DEPENDENCIES: std::sync::OnceLock<Box<[PortKey]>> = std::sync::OnceLock::new();
        DEPENDENCIES.get_or_init(|| {
            vec![PortKey::new(DATAFRAME_INPUT).expect("built-in port key is valid")]
                .into_boxed_slice()
        })
    }

    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError> {
        let input = PortAddress::declared(
            request.node_id,
            PortKey::new(DATAFRAME_INPUT).expect("built-in port key is valid"),
        );
        let schema = request.resolved_schemas.get(&input).ok_or_else(|| {
            InterfaceResolverError::new("dataframe input schema was not resolved")
        })?;

        schema.fields.iter().map(|field| {
            let (source, identity, guarantee) = match &field.lineage {
                Some(lineage) => (
                    lineage.source.clone(),
                    lineage.field.clone(),
                    SchemaFieldIdentityGuarantee::Stable,
                ),
                None => (
                    format!("snapshot:{}:{}", request.node_id, request.template.key).into(),
                    field.name.0.clone(),
                    SchemaFieldIdentityGuarantee::SnapshotScoped,
                ),
            };
            Ok(InterfaceResolverMember {
                basis: request.basis.clone(),
                locator: DynamicMemberLocator::SchemaField {
                    source: SchemaSourceIdentity(source),
                    field: SchemaFieldIdentity(identity),
                },
                label: field.name.0.to_string(),
                identity: guarantee,
            })
        }).collect::<Result<Vec<_>, InterfaceResolverError>>()
          .map(Vec::into_boxed_slice)
    }
}
```

Before returning, reject duplicate `(source, field)` locatorsan `InterfaceResolverError` containing a deterministic message. Do not deduplicate silently and do not use labels as stable identities.

- [ ] **Step 4: Assemble all built-in resolvers in one production builder**

Move only assembly responsibility into `compiler/dataframe.rs` or a small builder function in `compiler/mod.rs`:

```rust
pub fn build_builtin_interface_resolvers() -> InterfaceResolverSet {
    let mut resolvers = InterfaceResolverSet::new();
    project::install_function_interface_resolvers(&mut resolvers);
    dataframe::install_dataframe_interface_resolvers(&mut resolvers);
    resolvers
}
```

Refactor `project.rs` to expose:

```rust
pub(super) fn install_function_interface_resolvers(set: &mut InterfaceResolverSet)
```

and add:

```rust
pub(super) fn install_dataframe_interface_resolvers(set: &mut InterfaceResolverSet)
```

Every insertion must use `expect`a built-in uniqueness invariant. Keep `builtin_function_interface_resolver_ids()` for provider assembly; the DataFrame provider continues advertising its own ID.

- [ ] **Step 5: Run registration and built-in assembly tests**

Run:

```text
pnpm rust:test dataframe_columns_resolver_is_installed_in_builtin_set
pnpm rust:test node_system::catalog::tests
```

Expected: PASS,no duplicate provider resolver IDs.

- [ ] **Step 6: Write failing direct and transformed Decompose tests**

Using the built-in registry and `GraphCompiler::with_resolvers`, create table-driven cases whose source schema is:

```rust
[
    stable_field("customer_id", Int64),
    stable_field("region", String),
    stable_field("amount", Float64),
]
```

Cases and expected labels:

```text
source -> decompose                         [customer_id, region, amount]
source -> filter -> decompose               [customer_id, region, amount]
source -> project[amount, customer_id] -> decompose [amount, customer_id]
source -> rename customer_id=account_id -> decompose [account_id, region, amount]
```

Assert every case excludes:

```text
compiler.interface.resolver_missing
compiler.interface.resolver_failed
```

For Rename, assert the visible label is `account_id` while the locator remains:

```rust
DynamicMemberLocator::SchemaField {
    source: SchemaSourceIdentity("databases/main".into()),
    field: SchemaFieldIdentity("customer_id".into()),
}
```

- [ ] **Step 7: Run transformed tests and confirm GREEN after implementation**

Run:

```text
pnpm rust:test decompose_projects_final_upstream_schema
```

Expected: PASS for all four table entries.

- [ ] **Step 8: Add dynamic lifecycle tests**

Compile a documenta persisted binding for `customer_id`, then vary the source schema:

```text
unchanged schema -> same locator resolves the existing port
customer_id removed -> existing port status is Orphan
new total field -> total appears as unbound materialization candidate
same display labela different source/field locator -> does not reconnect
```

Assert no compile mutates `document.port_bindings`. Materialization remains authorization-driven through `ValidatedInterfaceProjection::materialization_candidate`.

- [ ] **Step 9: Run lifecycle tests**

Run:

```text
pnpm rust:test dataframe_decompose_preserves_exact_dynamic_field_identity
```

Expected: PASS.

---

### Task 4: Verify Production Compilation and Failure Diagnostics

**Files:**
- Modify: `src-tauri/src/project/production_tests.rs`
- Verify: `src-tauri/src/project/compile_publication.rs:321-331`
- Verify: `src-tauri/src/project/project_state.rs:8668-8678`
- Verify: `src-tauri/src/node_system/compiler/mod.rs`

**Interfaces:**
- Consumes: production `CompileResourceSnapshot::schema_resolvers()` and `build_builtin_interface_resolvers()`.
- Produces: regression proof that project compilation and function-plan publication both receive the complete resolver set and accurate database resource diagnostics.

- [ ] **Step 1: Write a failing production direct-database test**

Build a production `ProjectData` fixture with:

```text
database resource databases/main
schema: customer_id Int64, amount Float64
graph: Get DataFrame(dataframe="databases/main") -> Decompose DataFrame
```

Compile through the same project compile/publication helper used by editor projection. Assert:

```rust
assert!(!diagnostic_codes.contains(&"compiler.interface.resolver_missing"));
assert_eq!(decompose_candidate_labels, vec!["customer_id", "amount"]);
assert!(analysis.basis.resource_versions.contains_key(
    &ResourceKey::new("databases/main")
));
```

- [ ] **Step 2: Run the production test and verify behavior**

Run:

```text
pnpm rust:test project_compile_resolves_dataframe_decompose_columns
```

Expected before final wiring: FAILmissing resolver or absent candidates. Expected after Tasks 1-3: PASS.

- [ ] **Step 3: Add missing-database diagnostic coverage**

Use the same graph but omit `databases/main` from the production snapshot. Assert:

```rust
assert!(diagnostic_codes.contains(&"compiler.resource.resolution_failed"));
assert!(!diagnostic_codes.contains(&"compiler.interface.resolver_missing"));
assert!(!diagnostic_codes.contains(&"compiler.interface.resolver_failed"));
assert!(result.plan.is_none());
```

Also assert the diagnostic resource key is exactly `databases/main`.

- [ ] **Step 4: Run the missing-resource test**

Run:

```text
pnpm rust:test dataframe_decompose_preserves_missing_database_diagnostic
```

Expected: PASS. No successful empty-column interface is accepted.

- [ ] **Step 5: Verify both production builder call sites**

Confirm `compile_publication.rs::compile_and_publish` and `project_state.rs::publish_function_plans` still call the exported complete `build_builtin_interface_resolvers()`. Add a focused test only if either path bypasses the builder; do not duplicate builder construction in project code.

- [ ] **Step 6: Run focused regression suites**

Run serially:

```text
pnpm rust:test node_system::compiler::tests_dynamic
pnpm rust:test node_system::compiler::tests_dynamic_pipeline
pnpm rust:test node_system::catalog::dataframe::tests
pnpm rust:test project_compile_resolves_dataframe_decompose_columns
pnpm rust:test dataframe_decompose_preserves_missing_database_diagnostic
```

Expected: all PASS.

- [ ] **Step 7: Run required Rust and repository validation**

Run:

```text
pnpm rust:fmt:check
pnpm rust:check
git diff --check
```

Expected: all commands exit 0. Because this is a Rust-only change, do not run `pnpm verify` unless implementation unexpectedly changes frontend contracts.

- [ ] **Step 8: Review the final diff for scope and user-work safety**

Run:

```text
git --no-pager diff -- src-tauri/src/node_system/protocol/types.rs src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe src-tauri/src/project/project_state.rs src-tauri/src/project/production_tests.rs docs/superpowers/specs/2026-08-11-dataframe-decompose-schema-interface-design.md docs/superpowers/plans/2026-08-11-dataframe-decompose-schema-interface.md
```

Confirm the diff contains only schema lineage, staged interface resolution, DataFrame resolver registration/behavior, focused tests, and these documents. Do not revert or reformat unrelated dirty files.
