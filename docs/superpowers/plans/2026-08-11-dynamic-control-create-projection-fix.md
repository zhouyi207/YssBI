# Dynamic Control Create Projection Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep create-and-connect projection replacements parseable and preserve the persisted order of dynamic merge inputs.

**Architecture:** Rust remains the sole wire and graph-order authority. Correct the enum variant field serialization at its DTO definition, then make dynamic interface materialization emit ports in protocol traversal and persisted `OrderKey` order instead of UUID map order; keep the frontend parser unchanged and strict.

**Tech Stack:** Rust, Serde, Tauri mutation DTOs, Vitest/TypeScript wire parsers

## Global Constraints

- Do not add a frontend `node_id` compatibility path.
- Do not change mutation revision or control-flow lowering semantics.
- Preserve unrelated working-tree changes.
- Run Cargo with `--jobs 2` and Rust tests with `--test-threads=2`.
- Run Vitest with `--maxWorkers=2`.
- Do not create a Git commit unless explicitly requested.

---

## File Structure

- Modify `src-tauri/src/node_system/analysis/projection.rs`: own the canonical camelCase compilation-outcome wire.
- Modify `src-tauri/src/node_system/compiler/dynamic_interface.rs`: preserve protocol traversal and dynamic binding order in `ResolvedInterface.ports`.
- Modify `src-tauri/src/project/production_tests.rs`: convert the current diagnostic reproduction into permanent wire and ordering regression coverage.
- Validate `src/shared/types/dto/editorMutationWireParser.ts` without modifying it.

### Task 1: Correct the internal-failure projection wire

**Files:**
- Modify: `src-tauri/src/project/production_tests.rs:2030-2110`
- Modify: `src-tauri/src/node_system/analysis/projection.rs:85-95`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: `CompilationOutcomeDto::InternalFailure { stage, code, node_id }` and `GraphMutationResultDto` serialization.
- Produces: exact wire `{ "type": "internalFailure", "stage": ..., "code": ..., "nodeId": ... }`.

- [ ] **Step 1: Convert the diagnostic fixture into a failing wire test**

Rename `diagnostic_dynamic_merge_input_create_and_connect_projection_wire` to
`dynamic_merge_input_create_and_connect_serializes_parseable_internal_failure`.
Use fixed `PortInstanceId::from_uuid(uuid::Uuid::from_u128(...))` values, remove the
`println!`, serialize `result.projection_replacement.projection.outcome`, and assert:

```rust
let outcome = serde_json::to_value(
    &result.projection_replacement.projection.outcome,
)
.unwrap();
assert_eq!(outcome["type"], "internalFailure");
assert!(outcome.get("nodeId").is_some());
assert!(outcome.get("node_id").is_none());
assert_eq!(result.delta.to_revision, GraphRevision::new(2));
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
pnpm rust:test --jobs 2 --lib dynamic_merge_input_create_and_connect_serializes_parseable_internal_failure -- --test-threads=2
```

Expected: FAIL because the serialized object contains `node_id` and no `nodeId`.

- [ ] **Step 3: Apply the minimal Serde fix**

Change the enum annotation in `projection.rs` to rename variant fields as well as
variant names:

```rust
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CompilationOutcomeDto {
```

Do not rename the Rust field or relax the TypeScript parser.

- [ ] **Step 4: Verify GREEN for the wire regression**

Run the same focused command. Expected: PASS.

### Task 2: Preserve dynamic port document order

**Files:**
- Modify: `src-tauri/src/project/production_tests.rs:2030-2110`
- Modify: `src-tauri/src/node_system/compiler/dynamic_interface.rs:383-659`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: `DynamicPortBinding::{UserCreated, Resolved, Orphan}` persisted `OrderKey` and protocol-spec traversal order.
- Produces: `ResolvedInterface.ports: Box<[ResolvedPort<PortAddress>]>` in stable protocol/document order.

- [ ] **Step 1: Add a deterministic failing order assertion**

In the merge reproduction, assign the first persisted input a lexicographically later
UUID and the second input an earlier UUID:

```rust
let connected_instance = PortInstanceId::from_uuid(uuid::Uuid::from_u128(2));
let unconnected_instance = PortInstanceId::from_uuid(uuid::Uuid::from_u128(1));
```

Keep their bindings ordered as `00000` and `00001`. After the mutation, locate the
merge node in the returned projection, collect its instance `enter` IDs, and assert
that the `00000` address is first:

```rust
let merge_projection = result
    .projection_replacement
    .projection
    .nodes
    .iter()
    .find(|node| node.node_id.as_ref() == merge.id.to_string())
    .unwrap();
let enter_ids = merge_projection
    .ports
    .iter()
    .filter_map(|port| match &port.address {
        crate::node_system::document::PortAddressDto::Instance {
            template_key,
            instance_id,
            ..
        } if template_key.as_ref() == "enter" => Some(instance_id.as_ref()),
        _ => None,
    })
    .collect::<Vec<_>>();
assert_eq!(enter_ids, vec![connected_instance.to_string(), unconnected_instance.to_string()]);
```

Adjust the assertion to compare owned strings if required by the DTO field types.

- [ ] **Step 2: Run the focused test and verify RED**

Run the Task 1 focused command. Expected: FAIL with the two IDs reversed because
`MaterializationState::finish` currently calls `BTreeMap::into_values()`.

- [ ] **Step 3: Preserve insertion order in materialization**

Extend `MaterializationState` with an address sequence:

```rust
port_sequence: Vec<PortAddress>,
```

Initialize it empty and route every port insertion through a focused helper:

```rust
fn insert_port(&mut self, address: PortAddress, port: ResolvedPort<PortAddress>) {
    if !self.ports.contains_key(&address) {
        self.port_sequence.push(address.clone());
    }
    self.ports.insert(address, port);
}
```

Before iterating existing bindings for one spec, sort them by persisted binding order,
then by address as a deterministic tie-breaker:

```rust
bindings.sort_by(|(left_address, left_binding), (right_address, right_binding)| {
    dynamic_binding_order(left_binding)
        .cmp(dynamic_binding_order(right_binding))
        .then_with(|| left_address.cmp(right_address))
});
```

Add a private helper returning `&OrderKey` for all three binding variants. Replace the
five direct `self.ports.insert(...)` paths with `self.insert_port(...)`. In `finish`,
consume `port_sequence` and remove matching values from `ports`:

```rust
let mut ports = self.ports;
let ordered_ports = self
    .port_sequence
    .into_iter()
    .filter_map(|address| ports.remove(&address))
    .collect::<Vec<_>>()
    .into_boxed_slice();
```

Use `ordered_ports` for `ResolvedInterface.ports`. This preserves protocol traversal
for declared/template groups and `OrderKey` within dynamic instances.

- [ ] **Step 4: Verify GREEN for both regressions**

Run:

```sh
pnpm rust:test --jobs 2 --lib dynamic_merge_input_create_and_connect_serializes_parseable_internal_failure -- --test-threads=2
```

Expected: PASS.

### Task 3: Focused contract and project validation

**Files:**
- Validate: `src-tauri/src/node_system/analysis/projection.rs`
- Validate: `src-tauri/src/node_system/compiler/dynamic_interface.rs`
- Validate: `src-tauri/src/project/production_tests.rs`
- Validate: `src/shared/types/dto/editorMutationWireParser.ts`

**Interfaces:**
- Consumes: corrected Rust wire and resolved-port order.
- Produces: evidence that current frontend strict parsing and neighboring Rust contracts remain compatible.

- [ ] **Step 1: Run Rust formatting check**

```sh
pnpm rust:fmt:check
```

Expected: PASS.

- [ ] **Step 2: Run focused Rust contracts**

```sh
pnpm rust:test --jobs 2 --lib checked_in_node_system_contracts_match_rust -- --test-threads=2
```

Expected: PASS.

- [ ] **Step 3: Run the focused frontend wire tests**

```sh
pnpm test src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/shared/types/dto/editorMutationWireParser.test.ts --maxWorkers=2
```

Expected: 2 files and 43 tests pass.

- [ ] **Step 4: Run Rust check with limited jobs**

```sh
pnpm rust:check --jobs 2
```

Expected: PASS; existing warnings may remain but no errors.

- [ ] **Step 5: Check patch whitespace**

```sh
git diff --check
```

Expected: no output and exit code 0.
