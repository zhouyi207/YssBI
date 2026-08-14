# Node Instance Metadata and Inline Constants Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Rust project authoritative resource names and schema metadata into editor node titles and DataFrame pins, and add protocol-driven inline editing for Boolean, Int64, Float64, and String constants.

**Architecture:** Extend node protocols with explicit instance-display and parameter-presentation policies. Resolve resource names and dynamic member types during compilation, carry them through `AnalysisSnapshot` and editor DTOs, and make React render only the projected title, subtitle, pin metadata, and inline editor declarations. Keep all writes on the existing `setNodeParameters` revisioned mutation path.

**Tech Stack:** Rust, serde, Tauri editor projection DTOs, React 19, TypeScript, Zustand, shadcn/ui, Vitest, Cargo tests, pnpm.

## Global Constraints

- Rust remains authoritative for node protocols, resource identity, resource names, schema fields, pin types, diagnostics, and graph parameter values.
- React must not parse opaque resource paths, join resource stores to nodes, infer dynamic pin metadata, detect constants from `NodeTypeId`/category/key strings, or directly mutate graph entities.
- Resource names are primary node titles; `DocumentNode.user_label` is an independent subtitle.
- Resource display reads must participate in `AnalysisResourceReads`, observations, and compilation basis.
- `Decompose DataFrame` automatically projects every current field in schema order.
- Known schema types must project exactly; `RelationalScalarType::Unknown` projects as `Any` with a structured diagnostic.
- Orphan schema pins retain their connections, last known label, and last known structured type; no automatic migration is allowed.
- Constant values remain the existing `value` parameter and use the existing revisioned `setNodeParameters` mutation flow.
- Boolean constants submit immediately; numeric and string constants submit on Enter or blur; Escape cancels the draft.
- Use existing dependencies and shadcn/ui controls; add no UI library or package.
- Run commands from the repository root through `pnpm`; do not create `src-tauri/target/`.
- Preserve unrelated working-tree changes.
- Do not create commits unless the user explicitly authorizes commits. Commit checkpoints below are conditional reminders only.

## File Structure

### Rust protocol and catalog

- Modify `src-tauri/src/node_system/protocol/model.rs` — define resource-backed instance display policy on `NodeProtocol`.
- Modify `src-tauri/src/node_system/protocol/parameter.rs` — define detail-only versus inline-and-detail parameter presentation.
- Modify `src-tauri/src/node_system/protocol/mod.rs` — export new protocol types.
- Modify `src-tauri/src/node_system/registry/validation.rs` — validate display policy parameter existence and kind.
- Modify `src-tauri/src/node_system/registry/tests.rs` — protocol policy validation regressions.
- Modify `src-tauri/src/node_system/catalog/builtin.rs` — constant editor kinds and inline presentation.
- Modify `src-tauri/src/node_system/catalog/project.rs` — function and variable instance display declarations.
- Modify `src-tauri/src/node_system/catalog/dataframe/mod.rs` — database source instance display declaration.
- Modify other `src-tauri/src/node_system/catalog/**/*.rs` constructors only as required to default their policies to static/detail-only.

### Rust analysis, resources, and dynamic interface

- Modify `src-tauri/src/node_system/analysis/basis.rs` — include authoritative names in resolved function/database values.
- Modify `src-tauri/src/node_system/analysis/snapshot.rs` — carry resolved instance title and resolved port value type in analysis products.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs` — resolve instance display names, use per-port resolved value types, and snapshot them.
- Modify `src-tauri/src/project/project_state.rs` — snapshot function/database names with their versioned compile resources.
- Modify `src-tauri/src/node_system/compiler/dataframe.rs` — map schema scalar types into dynamic member types.
- Modify `src-tauri/src/node_system/compiler/dynamic_interface.rs` — carry member labels/types into resolved ports and last-known metadata.
- Modify `src-tauri/src/node_system/document/model.rs` — persist optional last-known structured type for orphan pins.
- Modify `src-tauri/src/node_system/compiler/diagnostics.rs` — add unsupported DataFrame field type diagnostic.
- Modify `src-tauri/src/node_system/catalog/dataframe/tests.rs` — label/order/type/orphan regressions.
- Modify `src-tauri/src/node_system/compiler/tests_dynamic.rs` and `src-tauri/src/node_system/compiler/tests_dynamic_pipeline.rs` — update member fixtures and test type propagation.
- Modify `src-tauri/src/project/production_tests.rs` — production snapshot title/rename/schema regressions.

### Editor projection and frontend contract

- Modify `src-tauri/src/node_system/analysis/projection.rs` — project analyzed title, dynamic instance label/type, orphan type, and parameter presentation.
- Modify `src/shared/types/dto/editorProjection.ts` — add `ParameterPresentationDto` and structured `valueType` to `ParameterEditorDto`.
- Modify `src/shared/types/dto/editorProjectionGuards.ts` — strictly validate the new field.
- Modify `src/shared/types/dto/editorMutationWireParser.ts` — strictly validate the new field on mutation responses.
- Modify DTO fixtures/tests under `src/features/domain/editorProjection/`, `src/services/nodeSystem/`, and `src/shared/types/dto/` to include the exact new field.
- Modify `src/features/core/dataStore/nodeView.ts` and `src/features/core/dataStore/useNodeView.ts` — remove title override support and resource-store joins.
- Modify `src/views/EditorView/Nodes/DefaultNodeLayout.tsx` — remove resource-store pin overrides and render projected subtitle/inline editors.
- Modify `src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx` — prove projection-only rendering.

### Inline constant editor

- Create `src/views/EditorView/Nodes/InlineParameterEditor.tsx` — render projected inline controls and own only active local drafts.
- Create `src/views/EditorView/Nodes/InlineParameterEditor.test.tsx` — interaction, parsing, mutation, rollback, and event-isolation tests.
- Modify `src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.tsx` — make ordinary toggle/number/text parameters editable in the detail panel through the same mutation helper.
- Add or update focused detail-editor tests beside `NodeParameterEditor.tsx`.

---

### Task 1: Add Explicit Protocol Display and Parameter Presentation Policies

**Files:**
- Modify: `src-tauri/src/node_system/protocol/model.rs`
- Modify: `src-tauri/src/node_system/protocol/parameter.rs`
- Modify: `src-tauri/src/node_system/protocol/mod.rs`
- Modify: `src-tauri/src/node_system/registry/validation.rs`
- Modify: `src-tauri/src/node_system/registry/tests.rs`
- Modify: `src-tauri/src/node_system/catalog/builtin.rs`
- Modify: `src-tauri/src/node_system/catalog/project.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Modify: affected node protocol constructors under `src-tauri/src/node_system/catalog/`

**Interfaces:**
- Produces: `NodeInstanceDisplaySpec`, `ResourceDisplayKind`, `ParameterPresentation`.
- Produces: `NodeProtocol.instance_display: NodeInstanceDisplaySpec`.
- Produces: `ParameterSpec.presentation: ParameterPresentation`.
- Consumes: existing `ParameterKey`, `ParameterEditorSpec`, and resource parameters represented as `core.string` plus `ParameterEditorSpec::Resource`.

- [ ] **Step 1: Write failing registry tests for valid and invalid instance display declarations**

Add focused builders/tests in `src-tauri/src/node_system/registry/tests.rs` that assert:

```rust
#[test]
fn resource_instance_display_requires_an_existing_resource_parameter() {
    let mut protocol = valid_test_protocol("yssbi.test.resource-title");
    protocol.instance_display = NodeInstanceDisplaySpec::ResourceParameter {
        parameter: ParameterKey::new("target").unwrap(),
        kind: ResourceDisplayKind::Function,
    };

    let error = validate_single_protocol(protocol).unwrap_err();
    assert!(matches!(
        error,
        RegistryValidationError::InvalidNode { reason, .. }
            if reason.contains("instance display parameter 'target'")
    ));
}

#[test]
fn resource_instance_display_rejects_a_non_resource_editor() {
    let mut protocol = valid_test_protocol("yssbi.test.resource-title");
    protocol.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: ParameterKey::new("target").unwrap(),
        title_key: I18nKey::new("test.parameter.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Text { multiline: false },
        presentation: ParameterPresentation::DetailPanel,
    }]).unwrap();
    protocol.instance_display = NodeInstanceDisplaySpec::ResourceParameter {
        parameter: ParameterKey::new("target").unwrap(),
        kind: ResourceDisplayKind::Function,
    };

    let error = validate_single_protocol(protocol).unwrap_err();
    assert!(matches!(
        error,
        RegistryValidationError::InvalidNode { reason, .. }
            if reason.contains("must use the resource editor")
    ));
}
```

Use the existing registry test fixture names if they differ; preserve the exact assertions and semantics above.

- [ ] **Step 2: Run the new tests and verify RED**

Run:

```text
pnpm rust:test --lib resource_instance_display
```

Expected: compilation fails because `NodeInstanceDisplaySpec`, `ResourceDisplayKind`, `ParameterPresentation`, and the new fields do not exist.

- [ ] **Step 3: Add the protocol enums and defaults**

In `protocol/model.rs`, add serializable protocol types:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodeInstanceDisplaySpec {
    #[default]
    Static,
    ResourceParameter {
        parameter: ParameterKey,
        kind: ResourceDisplayKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceDisplayKind {
    Function,
    Variable,
    Database,
}
```

Import `ParameterKey` and add this field to `NodeProtocol`:

```rust
#[serde(default)]
pub instance_display: NodeInstanceDisplaySpec,
```

In `protocol/parameter.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterPresentation {
    #[default]
    DetailPanel,
    InlineAndDetail,
}
```

Add to `ParameterSpec`:

```rust
#[serde(default)]
pub presentation: ParameterPresentation,
```

Export all three types from `protocol/mod.rs`.

- [ ] **Step 4: Update constructors with explicit defaults and validate resource display parameters**

Update every `NodeProtocol { ... }` constructor to set:

```rust
instance_display: NodeInstanceDisplaySpec::Static,
```

Update every `ParameterSpec { ... }` constructor to set:

```rust
presentation: ParameterPresentation::DetailPanel,
```

In `registry/validation.rs`, after parameter type validation, validate `NodeInstanceDisplaySpec::ResourceParameter` by finding the exact `ParameterKey` and requiring `ParameterEditorSpec::Resource`. Keep resource-kind interpretation for compilation; do not infer it from the path or parameter name.

Declare built-in identity policies:

```rust
// function call
NodeInstanceDisplaySpec::ResourceParameter {
    parameter: ParameterKey::new("target")?,
    kind: ResourceDisplayKind::Function,
}

// variable get/set
NodeInstanceDisplaySpec::ResourceParameter {
    parameter: ParameterKey::new("variable")?,
    kind: ResourceDisplayKind::Variable,
}

// dataframe source
NodeInstanceDisplaySpec::ResourceParameter {
    parameter: ParameterKey::new("dataframe")?,
    kind: ResourceDisplayKind::Database,
}
```

Keep managed function entry/return static in this slice because their identity belongs to the containing graph rather than an ordinary resource-bound instance.

- [ ] **Step 5: Declare exact constant editor and presentation kinds**

Change `constant_protocol` to receive or derive the editor kind from `ty`:

```rust
let editor = match ty {
    "core.bool" => ParameterEditorSpec::Toggle,
    "core.int64" | "core.float64" => ParameterEditorSpec::Number,
    "core.string" => ParameterEditorSpec::Text { multiline: false },
    _ => return Err(BuiltinAssemblyError::InvalidStaticDefinition(
        format!("unsupported constant type '{ty}'").into(),
    )),
};
```

Set:

```rust
editor,
presentation: ParameterPresentation::InlineAndDetail,
```

The built-in loop is closed over exactly these four constant types. Use `unreachable!("unsupported built-in constant type: {ty}")` for the exhaustive fallback; do not add a new assembly error variant.

- [ ] **Step 6: Run protocol/catalog focused tests**

Run:

```text
pnpm rust:test --lib resource_instance_display
pnpm rust:test --lib constant
pnpm rust:check
```

Expected: focused tests pass and `rust:check` exits 0; existing warning output is acceptable, new errors are not.

- [ ] **Step 7: Conditional commit checkpoint**

Only if the user has explicitly authorized commits:

```text
git add src-tauri/src/node_system/protocol src-tauri/src/node_system/registry src-tauri/src/node_system/catalog
git commit -m "Define node instance display policies"
```

Otherwise record the task as complete without committing.

---

### Task 2: Resolve Resource Names in the Compilation Snapshot and Project Titles

**Files:**
- Modify: `src-tauri/src/node_system/analysis/basis.rs`
- Modify: `src-tauri/src/node_system/analysis/snapshot.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Test: `src-tauri/src/node_system/analysis/projection.rs`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: `NodeProtocol.instance_display` from Task 1.
- Produces: `ResolvedFunctionValue.name: &'a str`.
- Produces: `ResolvedDatabaseValue<'a> { name: &'a str, columns: &'a [ColumnInfoDTO] }` and `ResolvedDatabase<'a> = ResolvedResource<ResolvedDatabaseValue<'a>>`.
- Produces: `AnalyzedNode.instance_title: Option<Box<str>>`.
- Produces: editor `NodeDisplayDto.title` selected from analyzed instance title, otherwise localized protocol title.

- [ ] **Step 1: Write failing projection tests for function, variable, and database titles**

In `analysis/projection.rs` tests, build one graph containing the three resource identity nodes with normalized canonical resource parameters and a `ResourceSnapshot` that returns names. Assert:

```rust
let titles = projection
    .nodes
    .iter()
    .map(|node| (node.node_type_id.as_ref(), node.display.title.as_ref()))
    .collect::<BTreeMap<_, _>>();
assert_eq!(titles["yssbi.project.function.call"], "Calculate Sales");
assert_eq!(titles["yssbi.project.variable.get"], "Revenue");
assert_eq!(titles["yssbi.dataframe.source.get"], "Sales Database");
assert_eq!(
    projection.nodes.iter()
        .find(|node| node.node_type_id.as_ref() == "yssbi.project.variable.get")
        .unwrap().display.user_label.as_deref(),
    Some("Previous period"),
);
```

Also assert a missing resource uses the localized protocol title and retains a resource diagnostic.

- [ ] **Step 2: Run the title test and verify RED**

Run:

```text
pnpm rust:test --lib resource_bound_editor_titles
```

Expected: FAIL because editor titles still come from `protocol.catalog.title_key`.

- [ ] **Step 3: Enrich resolved resource values without introducing untracked reads**

In `analysis/basis.rs` change resolved values to:

```rust
pub struct ResolvedFunctionValue<'a> {
    pub name: &'a str,
    pub function: &'a FunctionDocument,
    pub graph: &'a GraphDocument,
}

pub struct ResolvedDatabaseValue<'a> {
    pub name: &'a str,
    pub columns: &'a [crate::schema::ColumnInfoDTO],
}

pub type ResolvedDatabase<'a> = ResolvedResource<ResolvedDatabaseValue<'a>>;
```

Extend `ResourceSnapshot` in `compiler/pipeline.rs` with default methods:

```rust
fn function_name(&self, _path: &GraphResourcePath) -> Option<&str> { None }
fn database_name(&self, _id: &str) -> Option<&str> { None }
```

Require these values in `TrackedResourceResolver::resolve_function` and `resolve_database` before calling `successful`. Return a `ResourceResolutionError` if semantic data exists but the authoritative name is absent. Keep the same resource key/version, so the name read is part of the existing versioned resource observation.

Update test snapshots with names where they resolve those resources. Empty snapshots keep the default methods.

- [ ] **Step 4: Put function/database names in production compile snapshots**

Extend `CompileResourceSnapshot` with:

```rust
function_names: BTreeMap<GraphResourcePath, Box<str>>,
database_names: BTreeMap<ResourceId, Box<str>>,
```

Populate `function_names` from `ProjectData.graphs` using each `GraphResourceDocument.name` for function paths. Populate `database_names` from `ProjectData.databases`, using `DatabaseDecl.name` only when it is non-empty; missing names remain resolution failures and must not be derived from IDs.

Implement:

```rust
fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
    self.function_names.get(path).map(AsRef::as_ref)
}

fn database_name(&self, id: &str) -> Option<&str> {
    let key = ResourceId::new(format!("databases/{id}")).ok()?;
    self.database_names.get(&key).map(AsRef::as_ref)
}
```

Update every `CompileResourceSnapshot { ... }` literal, including tests, to initialize both maps.

- [ ] **Step 5: Resolve instance titles during analysis and snapshot them**

Add to `ResolvedNode` in `compiler/pipeline.rs`:

```rust
instance_title: Option<Box<str>>,
```

Add to `AnalyzedNode` in `analysis/snapshot.rs`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub instance_title: Option<Box<str>>,
```

Implement one focused compiler helper:

```rust
fn resolve_instance_title(
    &mut self,
    node_id: NodeId,
    protocol: &NodeProtocol,
    parameters: &BTreeMap<ParameterKey, serde_json::Value>,
    resources: &mut dyn AnalysisResourceResolver,
) -> Option<Box<str>>
```

Behavior:

- `Static` returns `None` without a resource read.
- `Function` parses the exact normalized string as `GraphResourcePath` and returns `resolved.value.name`.
- `Variable` requires the `variables/{VariableId}` canonical form, parses `VariableId`, and returns `resolved.value.name`.
- `Database` requires the `databases/{id}` canonical form and returns `resolved.value.name`.
- Parse/resolution failure pushes `CompilerDiagnostic::ResourceResolutionFailed` at `DiagnosticLocation::Node(node_id)` and returns `None`.
- No branch derives a name from a path.

Call this after parameter normalization and before inserting `ResolvedNode`. Copy the value into `AnalyzedNode.instance_title` in `snapshot()`.

- [ ] **Step 6: Make editor projection prefer analyzed titles**

In `analysis/projection.rs`, locate the matching `AnalyzedNode` already used for normalized parameters and select:

```rust
let title = normalized_node
    .and_then(|node| node.instance_title.clone())
    .unwrap_or_else(|| localization.text(
        &protocol.catalog.title_key,
        &DiagnosticArguments::new(),
    ));
```

Use this in `NodeDisplayDto.title`. Keep `node.user_label` unchanged as a separate field.

- [ ] **Step 7: Add production rename and fallback regressions**

In `project/production_tests.rs`, compile a graph with resource nodes, rename each underlying resource through the existing domain path, rebuild the production projection, and assert the new titles. Add a missing-name/database-name test that asserts static fallback plus diagnostic rather than an ID-derived title.

- [ ] **Step 8: Run title and production tests**

Run:

```text
pnpm rust:test --lib resource_bound_editor_titles
pnpm rust:test --lib resource_rename_updates_editor_title
pnpm rust:check
```

Expected: all focused tests pass and `rust:check` exits 0.

- [ ] **Step 9: Conditional commit checkpoint**

Only with explicit authorization:

```text
git add src-tauri/src/node_system/analysis src-tauri/src/node_system/compiler/pipeline.rs src-tauri/src/project/project_state.rs src-tauri/src/project/production_tests.rs
git commit -m "Project authoritative resource node titles"
```

---

### Task 3: Carry DataFrame Column Labels, Types, and Orphan Metadata End to End

**Files:**
- Modify: `src-tauri/src/node_system/analysis/snapshot.rs`
- Modify: `src-tauri/src/node_system/compiler/dataframe.rs`
- Modify: `src-tauri/src/node_system/compiler/dynamic_interface.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/diagnostics.rs`
- Modify: `src-tauri/src/node_system/document/model.rs`
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Test: `src-tauri/src/node_system/catalog/dataframe/tests.rs`
- Test: `src-tauri/src/node_system/compiler/tests_dynamic.rs`
- Test: `src-tauri/src/node_system/compiler/tests_dynamic_pipeline.rs`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Produces: `InterfaceResolverMember.value_type: TypeExpr`.
- Produces: `ResolvedPort.instance_label: Option<Box<str>>` and `ResolvedPort.value_type: TypeExpr`.
- Produces: `LastKnownPortMetadata.value_type: Option<TypeExpr>` with serde default/skip rules.
- Produces: `CompilerDiagnostic::DataframeFieldTypeUnsupported { column, schema_type, reason }`.
- Consumes: `RelationalScalarType` and concrete built-in type IDs.

- [ ] **Step 1: Write failing DataFrame tests for automatic labels, order, and all scalar types**

Extend `catalog/dataframe/tests.rs` with one table-driven test:

```rust
#[test]
fn decompose_projects_every_column_name_order_and_scalar_type() {
    let fields = vec![
        stable_field("active", RelationalScalarType::Boolean),
        stable_field("count", RelationalScalarType::Int64),
        stable_field("amount", RelationalScalarType::Float64),
        stable_field("name", RelationalScalarType::String),
        stable_field("day", RelationalScalarType::Date),
        stable_field("created", RelationalScalarType::DateTime),
    ];
    let result = compile_dataframe_document(&dataframe_document(None), fields.into_boxed_slice());
    let ports = decompose_resolved_ports(&result);

    assert_eq!(
        ports.iter().map(|port| port.instance_label.as_ref()).collect::<Vec<_>>(),
        vec!["active", "count", "amount", "name", "day", "created"],
    );
    assert_eq!(
        ports.iter().map(|port| port.data_type.clone()).collect::<Vec<_>>(),
        vec![
            DataType::Boolean,
            DataType::Int64,
            DataType::Float64,
            DataType::String,
            DataType::Date,
            DataType::Datetime,
        ],
    );
}
```

Use the existing compile result plus editor projection helper rather than inventing a frontend DTO fixture.

Add an Unknown test that expects `DataType::Any` and diagnostic code `compiler.dataframe.field_type_unsupported` with `column=opaque`.

- [ ] **Step 2: Write a failing orphan metadata test**

Build a document with a resolved binding, compile with an `Int64` field, then compile the same document with the field absent. Assert the projected orphan binding contains:

```rust
LastKnownPortMetadata {
    label: "customer_id".into(),
    value_type: Some(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
}
```

Also assert its existing connection remains in the editor projection.

- [ ] **Step 3: Run DataFrame tests and verify RED**

Run:

```text
pnpm rust:test --lib decompose_projects_every_column_name_order_and_scalar_type
pnpm rust:test --lib decompose_orphan_preserves_last_known_label_and_type
```

Expected: FAIL because member types and orphan types are not carried and instance labels still fall back to template metadata.

- [ ] **Step 4: Add a centralized scalar-to-node type mapping**

In `compiler/dataframe.rs`, add:

```rust
fn dataframe_field_type(field: &SchemaField) -> (TypeExpr, Option<CompilerDiagnostic>) {
    let concrete = |id: &str| TypeExpr::Concrete(TypeId::new(id).expect("built-in type ID"));
    match field.scalar_type {
        RelationalScalarType::Boolean => (concrete("core.bool"), None),
        RelationalScalarType::Int64 => (concrete("core.int64"), None),
        RelationalScalarType::Float64 => (concrete("core.float64"), None),
        RelationalScalarType::String => (concrete("core.string"), None),
        RelationalScalarType::Date => (concrete("core.date"), None),
        RelationalScalarType::DateTime => (concrete("core.datetime"), None),
        RelationalScalarType::Unknown => (
            TypeExpr::Unknown,
            Some(CompilerDiagnostic::DataframeFieldTypeUnsupported {
                column: field.name.0.clone(),
                schema_type: "unknown".into(),
                reason: "no concrete node type is registered for the schema field".into(),
            }),
        ),
    }
}
```

Confirm actual built-in date/datetime IDs in the type registry and use those exact IDs. The exhaustive match must have no wildcard arm.

Because interface resolvers currently return only members or one whole-interface error, replace the success value with this exact output type so one unsupported field can emit a diagnostic without failing the interface:

```rust
pub struct InterfaceResolverDiagnostic {
    pub locator: DynamicMemberLocator,
    pub diagnostic: CompilerDiagnostic,
}

pub struct InterfaceResolverOutput {
    pub members: Box<[InterfaceResolverMember]>,
    pub diagnostics: Box<[InterfaceResolverDiagnostic]>,
}
```

Change `InterfaceResolver::resolve` to return `Result<InterfaceResolverOutput, InterfaceResolverError>`, and adapt function resolvers/tests to return empty diagnostics. `DataframeColumnsResolver` returns one locator-bound diagnostic for each Unknown field.

- [ ] **Step 5: Carry value types on dynamic members and resolved ports**

Add to `InterfaceResolverMember`:

```rust
pub value_type: TypeExpr,
```

Function parameter/result resolvers populate this from their resolved function type. Generic test fixtures use the template's existing type where the test is not about member typing.

Add to `ResolvedPort` in `analysis/snapshot.rs`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub instance_label: Option<Box<str>>,
pub value_type: TypeExpr,
```

Replace `resolved_port(address, spec, status)` with:

```rust
fn resolved_port(
    address: PortAddress,
    spec: &PortSpec,
    instance_label: Option<Box<str>>,
    value_type: TypeExpr,
    status: ResolvedPortStatus,
) -> ResolvedPort<PortAddress>
```

Declared/user-created ports pass `None` and `spec.value_type.clone()`. Derived current members pass `Some(member.label.clone().into())` and `member.value_type.clone()`. Existing orphan ports pass the last-known label/type, falling back to `None` plus `spec.value_type.clone()` only for old data without type metadata.

Update compiler type analysis helpers to read `ResolvedPort.value_type` instead of re-reading `PortSpec.value_type` for instance ports. Keep `PortSpec` for direction, kind, connection, and editor capabilities.

- [ ] **Step 6: Persist and project last-known structured type**

Extend `LastKnownPortMetadata`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub value_type: Option<TypeExpr>,
```

When a current member matches an existing binding, store both its current label and type in `ProjectedDynamicPortBinding.last_known`. When it becomes orphaned, preserve that structure. Old serialized projects with label-only metadata deserialize with `None` and use the template fallback without migration code.

In `analysis/projection.rs`, set `PortDisplayDto.instance_label` directly from `ResolvedPort.instance_label`; the DTO builder does not receive `ValidatedInterfaceProjection` and must not reconstruct dynamic metadata. Resolve type from `analysis.partial_types`, which now originates from `ResolvedPort.value_type`; if no fact exists, project `ResolvedPort.value_type` directly. Update `project_data_type(TypeExpr::Unknown)` to return `Some(DataType::Any)` while `TypeSummaryDto.resolved` remains `false`, making the approved fallback explicit instead of producing a missing frontend type.

- [ ] **Step 7: Add the structured unsupported type diagnostic**

In `compiler/diagnostics.rs` add:

```rust
DataframeFieldTypeUnsupported { column, schema_type, reason } => {
    code: "compiler.dataframe.field_type_unsupported",
    message_key: "diagnostics.compiler.dataframe.field_type_unsupported",
    severity: Warning,
    en: "Column {column} uses unsupported schema type {schema_type}: {reason}.",
    zh: "列 {column} 使用了不支持的 Schema 类型 {schema_type}：{reason}。",
}
```

Attach it to `DiagnosticLocation::Port` for the projected field address when materializing resolver output. After deterministic addresses are allocated, build a locator-to-address map from the validated members, consume each `InterfaceResolverDiagnostic`, and push it at the matching port address. A diagnostic locator absent from validated members is an `InterfaceResolverError` because emitting an unlocatable field diagnostic would violate the resolver contract; do not attach field failures only to the node.

- [ ] **Step 8: Update all dynamic member fixtures and run focused suites**

Update every `InterfaceResolverMember { ... }` literal in:

- `compiler/project.rs`
- `compiler/tests_dynamic.rs`
- `compiler/tests_dynamic_pipeline.rs`
- DataFrame tests

Run:

```text
pnpm rust:test --lib dataframe_decompose
pnpm rust:test --lib dynamic_interface
pnpm rust:test --lib dataframe_field_type_unsupported
pnpm rust:check
```

Expected: all focused tests pass and known types project as concrete structured `DataType`, not `Any`.

- [ ] **Step 9: Add a production projection regression**

In `project/production_tests.rs`, compile a database source connected to Decompose using production `CompileResourceSnapshot`. Assert exact column labels, order, `Int64`/`Float64` types, and Unknown degradation diagnostic.

Run:

```text
pnpm rust:test --lib production_decompose_projects_database_column_metadata
```

Expected: PASS.

- [ ] **Step 10: Conditional commit checkpoint**

Only with explicit authorization:

```text
git add src-tauri/src/node_system/analysis src-tauri/src/node_system/compiler src-tauri/src/node_system/document src-tauri/src/node_system/catalog/dataframe src-tauri/src/project/production_tests.rs
git commit -m "Project typed DataFrame decomposition pins"
```

---

### Task 4: Extend the Strict Editor DTO Contract and Remove Frontend Resource Inference

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src/shared/types/dto/editorProjection.ts`
- Modify: `src/shared/types/dto/editorProjectionGuards.ts`
- Modify: `src/shared/types/dto/editorMutationWireParser.ts`
- Modify: `src/features/domain/editorProjection/editorProjection.test.ts`
- Modify: `src/services/nodeSystem/graphProjectionService.test.ts`
- Modify: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`
- Modify: `src/shared/types/dto/editorMutationWireParser.test.ts`
- Modify: `src/features/core/dataStore/nodeView.ts`
- Modify: `src/features/core/dataStore/nodeView.test.ts`
- Modify: `src/features/core/dataStore/useNodeView.ts`
- Modify: `src/features/core/dataStore/useNodeView.test.tsx`
- Modify: `src/views/EditorView/Nodes/DefaultNodeLayout.tsx`
- Modify: `src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx`

**Interfaces:**
- Consumes: `ParameterSpec.presentation` from Task 1.
- Produces: Rust `ParameterPresentationDto::{DetailPanel, InlineAndDetail}`.
- Produces: TypeScript `ParameterPresentationDto = 'detailPanel' | 'inlineAndDetail'`.
- Produces: `ParameterEditorDto.presentation` and `ParameterEditorDto.value_type` on every editor.
- Removes: `ToUiNodeOptions.title`, ResourceStore call-title override, variable/database store pin-name overrides, and category-based constant detection.

- [ ] **Step 1: Write failing strict-contract tests for presentation**

Update one canonical editor projection fixture to include:

```ts
presentation: 'inlineAndDetail',
valueType: { kind: 'Int64' },
```

Add guard assertions:

```ts
expect(isEditorGraphProjectionDto(validProjection())).toBe(true);

const missing = structuredClone(validProjection()) as any;
delete missing.nodes[0].parameterEditors[0].presentation;
expect(isEditorGraphProjectionDto(missing)).toBe(false);

const invalid = structuredClone(validProjection()) as any;
invalid.nodes[0].parameterEditors[0].presentation = 'inlineOnly';
expect(isEditorGraphProjectionDto(invalid)).toBe(false);
```

Do the equivalent mutation-response parser test.

- [ ] **Step 2: Run DTO tests and verify RED**

Run:

```text
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts
```

Expected: FAIL because DTO types/guards do not accept or require `presentation`.

- [ ] **Step 3: Project and validate parameter presentation**

In Rust `analysis/projection.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterPresentationDto {
    DetailPanel,
    InlineAndDetail,
}
```

Add `presentation` and `value_type: Option<DataType>` to Rust `ParameterEditorDto`. Map presentation exhaustively and compute `value_type` with the existing `project_data_type(&parameter.value_type)`; protocol validation already guarantees registered concrete built-in types.

In TypeScript add:

```ts
export type ParameterPresentationDto = 'detailPanel' | 'inlineAndDetail';

export interface ParameterEditorDto {
  key: string;
  display: ParameterDisplayDto;
  editor: ParameterEditorKindDto;
  presentation: ParameterPresentationDto;
  valueType: DataType | null;
  multiline: boolean;
  value: unknown | null;
  configuration: SchemaAwareParameterEditorDto | null;
}
```

Update both strict guards to require the exact `presentation` and `valueType` keys, accept only the two presentation values, and validate `valueType` with the existing structured `DataType` guard or `null`. Update all fixtures explicitly; do not default missing wire data in React.

- [ ] **Step 4: Write failing rendering tests that forbid frontend joins**

Replace the existing resource-store test in `DefaultNodeLayout.test.tsx` with projection-authority tests:

```tsx
it('renders projected title subtitle and pin names without resource stores', () => {
  const node = projectedNode({
    title: 'Sales Database',
    userLabel: 'Prior period',
    pinName: 'amount',
  });
  act(() => root.render(<DefaultNodeLayout node={node} />));
  expect(screenText()).toContain('Sales Database');
  expect(screenText()).toContain('Prior period');
  expect(container.querySelector('[data-testid="pin-name"]')?.textContent).toBe('amount');
});
```

In `useNodeView.test.tsx`, seed a function call with projected title `Calculate Sales` and an empty/mismatched `ResourceStore`; assert the hook still returns `Calculate Sales`.

- [ ] **Step 5: Run rendering tests and verify RED**

Run:

```text
pnpm test -- src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx src/features/core/dataStore/useNodeView.test.tsx
```

Expected: at least the subtitle assertion fails, and existing frontend overrides may replace projected metadata.

- [ ] **Step 6: Remove all frontend resource-title/pin inference**

In `nodeView.ts` remove `ToUiNodeOptions.title` and always use:

```ts
const title = nodeData.display?.title ?? nodeData.title;
```

Copy projected fields needed by the node layout into `UINode`:

```ts
display: nodeData.display,
parameterEditors: nodeData.parameterEditors ?? [],
diagnostics: nodeData.diagnostics ?? [],
```

`UINode` currently does not inherit these projection fields from `DomainNode`. Add these exact fields to `src/shared/types/ui/editor.ts`:

```ts
display?: NodeDisplayDto;
parameterEditors?: ParameterEditorDto[];
diagnostics?: DiagnosticDto[];
```

Import the DTO types directly; do not retain legacy resource IDs for display.

In `useNodeView.ts` remove imports and subscriptions for `useResourceStore`, `isCallFunctionNodeType`, and `getFunctionResourceName`. Call:

```ts
return toUiNode(nodeData, { pins });
```

In `DefaultNodeLayout.tsx` remove:

- `useVariableStore` and `useDatabaseStore`
- `isVariableNodeType` and `isDatabaseResourceNodeType`
- `resolveResourcePin`
- `node.category?.[1] === "Constants"`
- `forceShowInput={isConstantNode}`

Use `node.inputs` and `node.outputs` exactly as projected. Render the subtitle only when non-empty:

```tsx
{node.display?.userLabel ? (
  <span className="text-[10px] font-normal opacity-70">
    {node.display.userLabel}
  </span>
) : null}
```

Keep the primary title as `node.title`.

- [ ] **Step 7: Run strict DTO and projection-authority frontend tests**

Run:

```text
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/features/core/dataStore/nodeView.test.ts src/features/core/dataStore/useNodeView.test.tsx src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx
pnpm typecheck
```

Expected: all listed tests pass and TypeScript emits no errors.

- [ ] **Step 8: Conditional commit checkpoint**

Only with explicit authorization:

```text
git add src-tauri/src/node_system/analysis/projection.rs src/shared/types src/features/core/dataStore src/views/EditorView/Nodes src/services/nodeSystem
git commit -m "Consume authoritative node projection metadata"
```

---

### Task 5: Add Inline Constant Controls and Reuse the Detail Mutation Path

**Files:**
- Create: `src/views/EditorView/Nodes/InlineParameterEditor.tsx`
- Create: `src/views/EditorView/Nodes/InlineParameterEditor.test.tsx`
- Modify: `src/views/EditorView/Nodes/DefaultNodeLayout.tsx`
- Modify: `src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx`
- Modify: `src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.tsx`
- Create or modify: `src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.test.tsx`
- Reuse: `src/features/application/editor/setNodeParameters.ts`
- Reuse: `src/components/ui/input.tsx`
- Reuse: `src/components/ui/switch.tsx`

**Interfaces:**
- Consumes: `ParameterEditorDto.presentation === 'inlineAndDetail'` and editor kinds `toggle`, `number`, `text`.
- Consumes: `setNodeParameters({ graphPath, nodeId, locale, parameters })`.
- Produces: `InlineParameterEditor` with props `{ graphPath, nodeId, locale, parameter }`.
- Produces: shared local parsing/commit behavior usable by inline and detail editors without adding graph state.

- [ ] **Step 1: Write failing Boolean immediate-submit test**

In `InlineParameterEditor.test.tsx`, mock only `setNodeParameters` and render a projected Boolean editor:

```tsx
const parameter: ParameterEditorDto = {
  key: 'value',
  display: { title: 'Value', description: 'Constant value' },
  editor: 'toggle',
  presentation: 'inlineAndDetail',
  multiline: false,
  value: false,
  configuration: null,
};

it('submits a Boolean toggle immediately through setNodeParameters', async () => {
  renderEditor(parameter);
  clickSwitch();
  await flushPromises();
  expect(setNodeParameters).toHaveBeenCalledWith({
    graphPath,
    nodeId,
    locale: 'en-US',
    parameters: { value: true },
  });
});
```

- [ ] **Step 2: Write failing number/text timing and Escape tests**

Cover these exact behaviors:

```tsx
it.each([
  ['number', 12, '34', 34],
  ['text', 'old', 'new', 'new'],
] as const)('commits %s on Enter and not per keystroke', async (...))

it('commits a numeric draft on blur', async () => { ... })
it('does not submit an empty or invalid numeric draft', async () => { ... })
it('restores the latest projected value on Escape', async () => { ... })
it('restores the projected value when mutation rejects', async () => { ... })
```

For Int64, include an assertion that `1.5` is rejected. Distinguish Int64 from Float64 only through `ParameterEditorDto.valueType` added in Task 4; do not infer integer semantics from the node type or current value.

- [ ] **Step 3: Write failing canvas-event isolation tests**

Attach pointer/key handlers to a parent and assert:

- `pointerdown` in the control does not reach the node drag handler.
- Enter does not reach the canvas key handler.
- Escape cancels the draft and does not reach the canvas key handler.

Do not register new global listeners.

- [ ] **Step 4: Run inline editor tests and verify RED**

Run:

```text
pnpm test -- src/views/EditorView/Nodes/InlineParameterEditor.test.tsx
```

Expected: FAIL because the component does not exist.

- [ ] **Step 5: Implement a focused inline editor component**

Create `InlineParameterEditor.tsx` with this public shape:

```tsx
interface InlineParameterEditorProps {
  graphPath: string;
  nodeId: string;
  locale: string;
  parameter: ParameterEditorDto;
}

export function InlineParameterEditor(props: InlineParameterEditorProps) { /* typed branches */ }
```

Implementation rules:

- Keep `draft` and `pending` in component-local state.
- Synchronize `draft` from `parameter.value` when not actively editing/pending.
- `toggle` calls `setNodeParameters` immediately.
- `number` parses according to projected `valueType`: integer for `Int64`, finite number for `Float64`.
- `text` submits the exact string.
- Enter calls commit, blur calls commit, Escape resets from `parameter.value`.
- If the submitted value equals the projected value, clear editing without a mutation.
- On rejection, reset from `parameter.value` and call the existing shared toast store with an error message.
- Stop pointer and keyboard propagation at the control boundary.
- Use existing shadcn `Input` and `Switch`.

Export and use this pure parser from the component module so inline and detail editors share identical numeric semantics:

```ts
export function parseInlineNumber(
  draft: string,
  valueType: DataType | null,
): { ok: true; value: number } | { ok: false; message: string }
```

Do not create an inline-parameter Zustand store.

- [ ] **Step 6: Render only protocol-declared inline parameters**

In `DefaultNodeLayout.tsx`, derive:

```ts
const inlineParameters = (node.parameterEditors ?? [])
  .filter((parameter) => parameter.presentation === 'inlineAndDetail');
```

Render them between header and pin rows:

```tsx
{inlineParameters.length > 0 ? (
  <div className="flex flex-col gap-1 border-b border-[var(--node-border)] px-2 py-1.5">
    {inlineParameters.map((parameter) => (
      <InlineParameterEditor
        key={parameter.key}
        graphPath={graphPath!}
        nodeId={node.id}
        locale={locale}
        parameter={parameter}
      />
    ))}
  </div>
) : null}
```

Obtain locale through the existing i18n hook in this view or pass it from `CanvasNode`; do not read a global variable. If `graphPath` is absent, render the projected value read-only and do not submit.

- [ ] **Step 7: Make ordinary detail parameters editable through the same function**

Extend `NodeParameterEditor.tsx` beyond schema-aware editors:

- `toggle`: shadcn `Switch`, immediate commit.
- `number`: shadcn `Input`, Enter/blur commit, Escape reset, use the same pure parser.
- `text`: shadcn `Input` or textarea according to `multiline`, Enter behavior only for single-line, blur commit.
- `auto`, `select`, and `resource`: retain current read-only behavior unless an existing dedicated editor already handles them.

Generalize its commit signature from:

```ts
(value: string[] | FilterPredicateDto)
```

to:

```ts
(value: unknown)
```

and continue sending exactly `{ [parameter.key]: value }` through `setNodeParameters`.

- [ ] **Step 8: Run inline/detail interaction suites**

Run:

```text
pnpm test -- src/views/EditorView/Nodes/InlineParameterEditor.test.tsx src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.test.tsx src/features/application/editor/setNodeParameters.test.ts
pnpm typecheck
```

Expected: all tests pass and TypeScript emits no errors.

- [ ] **Step 9: Conditional commit checkpoint**

Only with explicit authorization:

```text
git add src/views/EditorView/Nodes src/views/EditorView/Layout/Detail/node/parameterEditors src/features/application/editor/setNodeParameters.ts
git commit -m "Add inline constant value editing"
```

---

### Task 6: Audit Resource Nodes, Run Cross-layer Regressions, and Verify Delivery

**Files:**
- Modify: `src-tauri/src/node_system/catalog/tests.rs` or add focused audit tests beside catalog registration.
- Modify: `src-tauri/src/node_system/testing/contracts.rs` if golden contract fixtures require the new fields.
- Modify: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts` if generated golden DTOs change.
- Modify: `AGENTS.md` only if implementation changes the documented architecture; no update is expected for this design because it follows existing authority boundaries.
- Verify all files changed by Tasks 1–5.

**Interfaces:**
- Consumes: all prior task interfaces.
- Produces: explicit built-in resource-display audit and cross-layer regression evidence.

- [ ] **Step 1: Add a built-in resource display policy audit test**

Enumerate built-in protocols with `ParameterEditorSpec::Resource` and assert the intended classification:

```rust
let expected_identity = BTreeMap::from([
    ("yssbi.project.function.call", ("target", ResourceDisplayKind::Function)),
    ("yssbi.project.variable.get", ("variable", ResourceDisplayKind::Variable)),
    ("yssbi.project.variable.set", ("variable", ResourceDisplayKind::Variable)),
    ("yssbi.dataframe.source.get", ("dataframe", ResourceDisplayKind::Database)),
]);
```

For every built-in resource parameter, require either an entry in this map or an explicit `Static` classification listed in the test with a short semantic reason in the test case name/data. Fail on newly added unclassified resource parameters.

- [ ] **Step 2: Run the audit and verify it passes**

Run:

```text
pnpm rust:test --lib every_resource_parameter_has_an_explicit_instance_display_classification
```

Expected: PASS with all current built-ins classified.

- [ ] **Step 3: Run focused Rust regression suites**

Run:

```text
pnpm rust:test --lib resource_instance_display
pnpm rust:test --lib resource_bound_editor_titles
pnpm rust:test --lib dataframe_decompose
pnpm rust:test --lib dataframe_field_type_unsupported
pnpm rust:test --lib dynamic_interface
pnpm rust:test --lib constant
pnpm rust:check
```

Expected: every command exits 0. Record any pre-existing warnings separately; fix warnings introduced by this work.

- [ ] **Step 4: Run focused frontend regression suites**

Run:

```text
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/features/core/dataStore/graphDataStore.test.ts src/features/core/dataStore/nodeView.test.ts src/features/core/dataStore/useNodeView.test.tsx src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx src/views/EditorView/Nodes/InlineParameterEditor.test.tsx src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.test.tsx
pnpm typecheck
```

Expected: all tests pass and typecheck exits 0.

- [ ] **Step 5: Inspect the final diff for forbidden compatibility paths**

Run searches:

```text
git grep -n "useVariableStore\|useDatabaseStore\|useResourceStore" -- src/views/EditorView/Nodes src/features/core/dataStore/useNodeView.ts
git grep -n "isConstantNode\|category.*Constants\|isCallFunctionNodeType" -- src/views/EditorView/Nodes src/features/core/dataStore/useNodeView.ts
git grep -n "strip_prefix.*functions\|strip_prefix.*variables\|strip_prefix.*databases" -- src
```

Expected:

- First two commands return no matches in the listed node-rendering paths.
- The third command may return backend canonical resource parsing, but no frontend path parsing under `src/`.

Inspect `git diff` and verify no unrelated files or behavior were changed.

- [ ] **Step 6: Run formatting and whitespace checks**

Run:

```text
pnpm rust:fmt:check
git diff --check
```

Expected: both commands exit 0.

- [ ] **Step 7: Run the required cross-stack verification**

Run:

```text
pnpm verify
```

Expected: frontend typecheck/tests, Rust format/check/tests, SCI tests, and `git diff --check` all exit 0. If a known unrelated test fails, report the exact command and failure without changing unrelated code.

- [ ] **Step 8: Review acceptance criteria against fresh evidence**

Confirm from test output and the final diff:

- Resource identity nodes display authoritative resource names.
- User labels remain subtitles.
- Resource rename updates through a backend projection.
- React has no resource-title or resource-pin join logic.
- Decompose shows all columns in schema order with exact names and known types.
- Unknown types show `Any` plus a structured diagnostic.
- Orphans retain label, type, and connections.
- Four constant kinds are editable inline and in the detail panel.
- Boolean/Enter/blur/Escape behavior matches the approved design.
- All writes use `setNodeParameters` and backend projection events.

- [ ] **Step 9: Conditional final commit**

Only with explicit user authorization:

```text
git add src-tauri/src src docs/superpowers/specs/2026-08-12-node-instance-metadata-and-inline-constants-design.md docs/superpowers/plans/2026-08-12-node-instance-metadata-and-inline-constants.md
git commit -m "Fix node instance metadata and constants"
```

Without authorization, leave the verified changes uncommitted and report the changed files and validation evidence.
