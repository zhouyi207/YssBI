# Node Architecture Project Identity and Wire Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Batch A active-project node command lifecycle-safe and freeze graph/project-event plus execution wire contracts across Rust and TypeScript.

**Architecture:** Frontend application workflows capture one `ProjectIdentitySnapshot`, pass its required `projectInstanceId` through service DTOs, and reject stale completions. Rust validates the caller identity at the final `ProjectState` authority gate, returns project-scoped DTOs, and emits events carrying the same identity. Strict TypeScript parsers consume Rust golden fixtures before any store effect.

**Tech Stack:** Rust 2024, Tauri 2 commands/events/channels, Serde, React 19, TypeScript 5.8, Zustand, Vitest 4, pnpm 11.

## Global Constraints

- Follow `AGENTS.md`: Rust owns domain state and business logic; React stores are projections/UI state.
- `ProjectState.project_data` remains authoritative.
- Commands stay thin and services own frontend IPC.
- Required identity fields are strict; do not add optional compatibility fields or a second envelope.
- Resource paths remain opaque.
- Stale lifecycle rejection has zero state, revision, run-registry, event, and frontend-store effects.
- Preserve unrelated dirty work, including the current database revision/bootstrap fixes.
- Use RED-GREEN for every behavior change.
- Do not create commits, branches, worktrees, tags, or staging changes unless the user explicitly requests them.
- Run Rust tests serially when they use shared hooks or filesystem state.

---

## File map

### Rust authority and IPC

- `src-tauri/src/event/event_project.rs`: canonical graph mutation result and `GraphDelta` event DTOs.
- `src-tauri/src/commands/command_node_system.rs`: active-project Tauri command parameters, thin forwarding, emitters, command tests.
- `src-tauri/src/commands/node_system_execution_dto.rs`: execution demand/run-event wire DTOs and Rust exact-shape tests.
- `src-tauri/src/project/project_state.rs`: final graph/function/history/execution authority gates.
- `src-tauri/src/node_system/testing/contracts.rs`: checked-in Rust golden fixture generator.

### TypeScript IPC and application ownership

- `src/shared/types/dto/editorMutation.ts`: graph mutation project identity DTO.
- Create `src/shared/types/dto/editorMutationWireParser.ts`: strict graph delta/direct-result parser.
- `src/shared/types/dto/runEvent.ts`: execution DTO declarations consumed by the parser.
- Create `src/shared/types/dto/runEventParser.ts`: strict execution wire parser.
- Create `src/features/core/sync/utils/projectEventWireParser.ts`: strict `GraphDelta` and `ResourceMutationCommitted` envelope parser.
- `src/services/nodeSystem/graphMutationService.ts`: graph command IPC.
- `src/services/nodeSystem/functionMutationService.ts`: function-signature IPC.
- `src/services/nodeSystem/graphProjectionService.ts`: projection hydrate IPC.
- `src/services/nodeSystem/historyService.ts`: History status/undo/redo IPC.
- `src/services/project/projectService.ts`: execution IPC and channel parsing.
- `src/features/application/editorMutation/editorMutationCoordinator.ts`: graph mutation lifecycle snapshot.
- `src/features/application/editorMutation/functionSignatureCoordinator.ts`: function mutation lifecycle snapshot.
- `src/features/application/editorMutation/historyCoordinator.ts`: History lifecycle snapshot.
- `src/features/application/editor/requestPinPreview.ts`: preview execution lifecycle snapshot.
- `src/features/application/editor/useProjectOperations.ts`: normal execution lifecycle snapshot.
- `src/features/core/sync/handlers/ProjectMutationEventHandler.ts`: parse and reject stale events before store access.

### Golden fixtures and audits

- Create `src/tests/fixtures/node-system-contracts/project-events.json`.
- Create `src/tests/fixtures/node-system-contracts/execution-wire.json`.
- `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`: TypeScript fixture consumption.
- `src/services/project/projectFilesystemContract.test.ts`: active-project command identity policy audit.

---

### Task 1: Close the `GraphDelta` result and event identity wire

**Files:**
- Modify: `src-tauri/src/event/event_project.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src/shared/types/dto/editorMutation.ts`
- Create: `src/shared/types/dto/editorMutationWireParser.ts`
- Create: `src/shared/types/dto/editorMutationWireParser.test.ts`
- Create: `src/features/core/sync/utils/projectEventWireParser.ts`
- Create: `src/features/core/sync/utils/projectEventWireParser.test.ts`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.ts`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts`

**Interfaces:**
- Produces Rust `GraphMutationResultDto { project_instance_id, delta, projection_replacement, history }`.
- Produces Rust `EventProject::GraphDelta { project_instance_id, delta }` serialized as `{ type: "GraphDelta", payload: { projectInstanceId, delta } }`.
- Produces TS `parseGraphDeltaDto(value)`, `parseGraphMutationResultDto(value)`, `parseGraphDeltaEventPayload(value)`, and `parseResourceMutationCommittedPayload(value)`.
- Later tasks rely on the required `GraphMutationResultDto.projectInstanceId: string` and strict service parsing.

- [ ] **Step 1: Add a failing Rust exact-envelope test**

In `event_project.rs`, construct a fixed graph delta and assert the exact JSON:

```rust
assert_eq!(
    serde_json::to_value(EventProject::GraphDelta {
        project_instance_id: "project-a".into(),
        delta: delta.clone(),
    }).unwrap(),
    serde_json::json!({
        "type": "GraphDelta",
        "payload": {
            "projectInstanceId": "project-a",
            "delta": serde_json::to_value(delta).unwrap(),
        }
    }),
);
```

Also assert `GraphMutationResultDto` serializes a required `projectInstanceId` and rejects a fixture missing it.

- [ ] **Step 2: Run the Rust test and verify RED**

Run:

```sh
pnpm rust:test --lib graph_delta_event_carries_project_identity -- --nocapture
```

Expected: compile/test failure because the Rust variants do not yet contain `project_instance_id`.

- [ ] **Step 3: Add failing TypeScript parser and handler tests**

Define the wished-for strict parser behavior:

```ts
expect(parseGraphDeltaEventPayload({ projectInstanceId, delta })).toEqual({ projectInstanceId, delta });
expect(() => parseGraphDeltaEventPayload({ delta })).toThrow('projectInstanceId');
expect(() => parseGraphDeltaEventPayload({ projectInstanceId, delta, extra: true })).toThrow('exact');
expect(parseGraphMutationResultDto(graphResult).projectInstanceId).toBe(projectInstanceId);
expect(() => parseGraphMutationResultDto({ ...graphResult, projectInstanceId: undefined })).toThrow();
expect(parseResourceMutationCommittedPayload({ result: resourceResult })).toEqual({ result: resourceResult });
```

In `ProjectMutationEventHandler.test.ts`, use the exact Rust payload. Prove current identity calls `invalidateGraphProjection`, while stale identity does not read `useGraphDataStore.getState` or `getPendingMutation`.

- [ ] **Step 4: Run the TypeScript tests and verify RED**

Run:

```sh
pnpm test src/shared/types/dto/editorMutationWireParser.test.ts src/features/core/sync/utils/projectEventWireParser.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
```

Expected: FAIL because the parser does not exist and the production Rust-shaped payload is ignored.

- [ ] **Step 5: Implement the minimal Rust DTO changes**

Change the DTOs to required fields:

```rust
pub struct GraphMutationResultDto {
    pub project_instance_id: String,
    pub delta: GraphDeltaEvent<GraphDocumentPatch>,
    pub projection_replacement: GraphProjectionReplacementDto,
    pub history: HistoryStatusDto,
}

GraphDelta {
    project_instance_id: String,
    delta: GraphDeltaEvent<GraphDocumentPatch>,
},
```

Populate both fields from the validated current publication/session identity at the same authority gate that constructs the delta. Update the emitter to copy `result.project_instance_id` rather than request input.

- [ ] **Step 6: Implement the exact TypeScript parser and handler integration**

Use record/exact-key guards:

```ts
export function parseGraphDeltaEventPayload(value: unknown): GraphDeltaEventPayload {
  if (!isRecord(value) || !hasExactKeys(value, ['projectInstanceId', 'delta'])) {
    throw new Error('GraphDelta payload must have exact projectInstanceId and delta fields');
  }
  if (typeof value.projectInstanceId !== 'string' || value.projectInstanceId.length === 0) {
    throw new Error('GraphDelta projectInstanceId is malformed');
  }
  return { projectInstanceId: value.projectInstanceId, delta: parseGraphDeltaDto(value.delta) };
}
```

No reusable `parseGraphDeltaDto` exists. Implement it in `editorMutationWireParser.ts`; validate exact graph path, safe integer revisions, operation UUID/null, and every graph patch operation. Build `parseGraphMutationResultDto` from that validator plus exact projection replacement and History validation. Do not cast raw `unknown` before validation.

`projectEventWireParser.ts` validates exact event envelope keys, delegates graph result/delta validation to the shared DTO parser, and delegates resource mutation validation to `validateResourceMutationWireResult`. Call these parsers at the beginning of both project mutation handlers; only after parsing call `isCurrentProjectEvent`, publication coordination, pending lookup, or stores. On a malformed current-project graph event, mark only a safely extracted valid graph path stale; never apply partial payload state.

- [ ] **Step 7: Run focused tests and verify GREEN**

Run both commands from Steps 2 and 4. Expected: all selected tests PASS.

- [ ] **Step 8: Review checkpoint**

Inspect `git diff --check` and confirm the direct result and event obtain identity from committed backend authority, not from unvalidated frontend input.

---

### Task 2: Make graph mutation lifecycle-owned end to end

**Files:**
- Modify: `src/services/nodeSystem/graphMutationService.ts`
- Create: `src/services/nodeSystem/graphMutationService.test.ts`
- Modify: `src/features/application/editorMutation/editorMutationCoordinator.ts`
- Modify: `src/features/application/editorMutation/editorMutationCoordinator.test.ts`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/project/project_state.rs`

**Interfaces:**
- Changes service signature to `mutateGraph(projectInstanceId, graphPath, locale, request)`.
- Changes coordinator dependency to the same signature.
- Rust command accepts `project_instance_id: ProjectInstanceId`.
- ProjectState mutation entry accepts `&ProjectInstanceId` and validates it at final commit.

- [ ] **Step 1: Add failing frontend lifecycle tests**

Add tests proving one captured identity is passed and stale completion is ignored:

```ts
const completion = executeEditorMutation(input, { mutateGraph });
expect(mutateGraph).toHaveBeenCalledWith(projectInstanceId, graphPath, locale, expect.any(Object));
projectPublicationCoordinator.startProject(replacementId, 0);
result.resolve(graphMutationResult);
await expect(completion).resolves.toEqual({ status: 'stale', result: graphMutationResult });
expect(applyStoreEffect).not.toHaveBeenCalled();
```

Also trigger project replacement while reading graph revision authority and prove no command is invoked.

- [ ] **Step 2: Run frontend test and verify RED**

```sh
pnpm test src/features/application/editorMutation/editorMutationCoordinator.test.ts src/services/nodeSystem/graphMutationService.test.ts
```

Expected: FAIL because `projectInstanceId` is not in the service/dependency signature and stale completion still reaches `applyMutationResult`.

- [ ] **Step 3: Add failing Rust stale/race tests**

Cover both stale-before-entry and replacement-before-finalize:

```rust
let stale_id = state.capture_project_session().unwrap().instance_id;
state.activate_project_fixture(other_root, other_project);
let before = graph_authority_effects(&state);
let result = mutate_graph_document_with_emitter(
    &state,
    stale_id,
    graph_path,
    "en-US",
    request,
    |event| events.push(event),
);
assert_eq!(result.unwrap_err().code, "stale_project_lifecycle");
assert_eq!(graph_authority_effects(&state), before);
assert!(events.is_empty());
```

Use `ProjectState::set_mutation_publication_test_hook` to pause before publication, replace the project, release the hook, and assert the same zero-effect outcome.

- [ ] **Step 4: Run Rust tests and verify RED**

```sh
pnpm rust:test --lib graph_mutation_rejects_stale_caller_project -- --nocapture
pnpm rust:test --lib graph_mutation_rejects_project_replacement_before_finalize -- --nocapture
```

Expected: FAIL because the command/domain path has no caller project identity.

- [ ] **Step 5: Implement frontend identity capture**

At the start of `executeEditorMutation`, capture identity before reading projection basis:

```ts
const identity = captureProjectIdentity();
const basis = getGraphProjectionBasis(...);
assertCurrentProjectIdentity(identity);
```

Pass `identity.projectInstanceId` through the dependency/service. `GraphMutationService` invokes as `unknown` and returns `parseGraphMutationResultDto(response)`. After await, return stale if `!isCurrentProjectIdentity(identity)` before calling `applyMutationResult`, hydrate, History update, or store code. Treat backend `stale_project_lifecycle` as `{ status: 'stale' }`.

- [ ] **Step 6: Implement Rust entry and final-gate validation**

Thread `ProjectInstanceId` through:

```rust
pub fn mutate_graph_document(
    ...,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    ...,
)
```

The ProjectState operation must compare the expected project ID under the same final publication/document lock set used to commit the patch. Return the existing typed stale lifecycle error before any revision allocation or event callback.

- [ ] **Step 7: Run all Task 2 tests and verify GREEN**

Run Steps 2 and 4. Expected: PASS with zero-effect assertions.

- [ ] **Step 8: Review checkpoint**

Confirm no `projectInstanceId?:` optional DTO was introduced and no command-only check leaves a TOCTOU commit path.

---

### Task 3: Close function-signature and projection-hydrate identity

**Files:**
- Modify: `src/services/nodeSystem/functionMutationService.test.ts`
- Modify: `src/services/nodeSystem/graphProjectionService.test.ts`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.ts`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.test.ts`
- Modify: `src/features/application/editorProjection/graphProjectionCoordinator.ts`
- Modify: `src/features/application/editorProjection/graphProjectionCoordinator.test.ts`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/project/project_state.rs`

**Interfaces:**
- Existing TypeScript services already send identity; Rust must consume it.
- Rust `update_function_signature` and `hydrate_editor_graph` accept required `ProjectInstanceId`.
- Function/projection ProjectState paths validate expected identity at snapshot and final commit.

- [ ] **Step 1: Strengthen existing service tests**

Keep exact `invoke` assertions and add rejection fixtures showing the service does not remove the identity field. These tests should initially PASS and serve as the fixed caller contract.

- [ ] **Step 2: Add failing Rust command decode/authority tests**

Invoke helper functions with stale IDs and assert:

```rust
assert_eq!(error.code, "stale_project_lifecycle");
assert!(events.is_empty());
assert_eq!(state.get_data().unwrap(), before_data);
```

For hydrate, replace the project after capturing the old ID and assert no projection from the replacement project is returned.

- [ ] **Step 3: Run Rust tests and verify RED**

```sh
pnpm rust:test --lib function_signature_command_rejects_stale_project_identity -- --nocapture
pnpm rust:test --lib hydrate_editor_graph_rejects_stale_project_identity -- --nocapture
```

Expected: FAIL because Rust command signatures ignore the frontend field.

- [ ] **Step 4: Implement required Rust parameters and authority checks**

Add `project_instance_id: ProjectInstanceId` to both Tauri commands. For hydrate, call an identity-aware projection method rather than `graph_projection(path, locale)`. For signature updates, validate at the final publication gate and keep `ResourceMutationResultDto.project_instance_id` sourced from authority.

- [ ] **Step 5: Add frontend replacement-race tests**

In both coordinators, delay the service response, replace project lifecycle, resolve, and assert stale/no store installation. Use existing `ProjectIdentitySnapshot` helpers rather than coordinator-local epoch substitutes.

- [ ] **Step 6: Run focused frontend and Rust tests and verify GREEN**

```sh
pnpm test src/services/nodeSystem/functionMutationService.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/editorProjection/graphProjectionCoordinator.test.ts
pnpm rust:test --lib function_signature_command_rejects_stale_project_identity -- --nocapture
pnpm rust:test --lib hydrate_editor_graph_rejects_stale_project_identity -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Review checkpoint**

Confirm frontend-sent identity is consumed by Rust and no projection can cross project replacement.

---

### Task 4: Make History status, undo, and redo lifecycle-owned

**Files:**
- Modify: `src/services/nodeSystem/historyService.ts`
- Create or modify: `src/services/nodeSystem/historyService.test.ts`
- Modify: `src/features/application/editorMutation/historyCoordinator.ts`
- Modify: `src/features/application/editorMutation/historyCoordinator.test.ts`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/project/project_state.rs`

**Interfaces:**
- `HistoryService.getStatus(projectInstanceId)`.
- `HistoryService.undo(projectInstanceId, locale, request)`.
- `HistoryService.redo(projectInstanceId, locale, request)`.
- Rust commands accept `ProjectInstanceId`; mutation results retain authority-issued identity.

- [ ] **Step 1: Add failing exact service tests**

```ts
await HistoryService.getStatus(projectInstanceId);
expect(invoke).toHaveBeenCalledWith('get_project_history_status', { projectInstanceId });
await HistoryService.undo(projectInstanceId, locale, request);
expect(invoke).toHaveBeenCalledWith('undo_graph_document', { projectInstanceId, locale, request });
```

Repeat for redo.

- [ ] **Step 2: Add failing coordinator lifecycle tests**

Replace project identity while status/undo/redo is in flight. Assert no History store update, no publication, no hydrate, and `{ status: 'stale' }` for mutations.

- [ ] **Step 3: Run frontend tests and verify RED**

```sh
pnpm test src/services/nodeSystem/historyService.test.ts src/features/application/editorMutation/historyCoordinator.test.ts
```

Expected: FAIL because service signatures lack identity and coordinator uses only its local reset epoch.

- [ ] **Step 4: Add failing Rust zero-effect tests**

Test stale status, stale undo/redo at entry, and replacement during History preparation/finalize. Capture project data, History head, resource revisions, publication revision, and emitted events before the operation; assert exact equality afterward.

- [ ] **Step 5: Run Rust tests and verify RED**

```sh
pnpm rust:test --lib history_commands_reject_stale_project_identity_with_zero_effects -- --nocapture
```

Expected: FAIL before implementation.

- [ ] **Step 6: Implement frontend and Rust identity threading**

Capture `ProjectIdentitySnapshot` before reading the anchor revision. Pass the ID through services. Replace coordinator-only epoch checks with `isCurrentProjectIdentity(identity)` for project ownership; retain coordinator epoch only for coordinator reset ownership.

Rust validates status reads against the expected session and threads the expected ID through undo/redo preparation and final authority comparison.

- [ ] **Step 7: Run Task 4 tests and verify GREEN**

Run Steps 3 and 5. Expected: PASS.

- [ ] **Step 8: Review checkpoint**

Confirm stale History results never enter `projectPublicationCoordinator.submit`.

---

### Task 5: Make execution lifecycle-owned before run registration

**Files:**
- Modify: `src/services/project/projectService.ts`
- Modify: `src/services/project/projectService.execution.test.ts`
- Modify: `src/features/application/editor/requestPinPreview.ts`
- Modify: `src/features/application/editor/requestPinPreview.test.ts`
- Modify: `src/features/application/editor/useProjectOperations.ts`
- Modify: `src/features/application/editor/useProjectOperations.execution.test.tsx`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/project/project_state.rs`

**Interfaces:**
- `ProjectService.executeGraphDocument(projectInstanceId, graphPath, demand, onEvent?)`.
- Rust command accepts `project_instance_id: ProjectInstanceId`.
- ProjectState exposes an identity-aware execution method that rejects before run insertion/event emission.

- [ ] **Step 1: Add failing service test**

Update exact invocation expectation:

```ts
expect(invoke).toHaveBeenCalledWith('execute_graph_document', {
  projectInstanceId,
  graphPath,
  demand,
  onEvent: expect.any(Channel),
});
```

- [ ] **Step 2: Add failing application stale tests**

For preview and normal run, delay execution, replace lifecycle, emit an event, and resolve. Assert stale events do not update execution/result stores and stale completion does not report success for the replacement project.

- [ ] **Step 3: Run frontend tests and verify RED**

```sh
pnpm test src/services/project/projectService.execution.test.ts src/features/application/editor/requestPinPreview.test.ts
```

Expected: FAIL because the service lacks identity.

- [ ] **Step 4: Add failing Rust execution zero-effect tests**

For stale-before-entry and replacement-before-registration assert:

```rust
assert_eq!(error.code, "stale_project_lifecycle");
assert!(events.is_empty());
assert_eq!(state.active_run_count_for_test(), 0);
assert_eq!(state.result_source_count_for_test(), 0);
```

In the `project_state.rs` module test, assert `state.runs.active_run_count() == 0` and `state.results.source_count() == 0`; both production-owned registries already expose these read-only count methods, so no new production test API is needed.

- [ ] **Step 5: Run Rust tests and verify RED**

```sh
pnpm rust:test --lib execute_graph_rejects_stale_project_before_run_registration -- --nocapture
```

Expected: FAIL because execution captures whichever project is current when the worker starts.

- [ ] **Step 6: Implement identity-aware execution**

Pass expected ID into the blocking closure and `ProjectState::execute_graph`. Validate it while capturing the execution snapshot and validate it again immediately before registering the run. Do not send a terminal run event for a command rejected before run creation; return plain `stale_project_lifecycle` without `terminalRunEventSent`.

Update both frontend workflows to pass their already captured identity and check it in channel callbacks and after completion.

- [ ] **Step 7: Run Task 5 tests and verify GREEN**

Run Steps 3 and 5 plus existing channel-drain tests. Expected: PASS with unchanged terminal-drain behavior for real runs.

- [ ] **Step 8: Review checkpoint**

Confirm opaque `runId`/`sourceId` follow-up APIs remain unchanged and execution rejection occurs before any run capability is allocated.

---

### Task 6: Freeze execution and project-event Rust↔TS contracts

**Files:**
- Modify: `src-tauri/src/node_system/testing/contracts.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Create: `src/tests/fixtures/node-system-contracts/project-events.json`
- Create: `src/tests/fixtures/node-system-contracts/execution-wire.json`
- Create: `src/shared/types/dto/runEventParser.ts`
- Create: `src/shared/types/dto/runEventParser.test.ts`
- Modify: `src/services/project/executionChannelDrain.ts`
- Modify: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`

**Interfaces:**
- Produces checked-in exact JSON fixtures for every event/execution variant.
- Produces `parseExecutionDemandDto`, `parseRunEvent`, and `parseExecuteGraphResultDto` production parsers.
- Execution channel dispatch consumes `parseRunEvent(raw)` before invoking application callbacks.

- [ ] **Step 1: Add failing Rust fixture completeness tests**

Build a table containing every `RunEventKindDto` variant and both `ExecutionDemandDto` variants. Assert stable names and exact keys. Include unsafe opaque IDs as decimal strings. Add a completeness assertion that fails when the enum conversion inventory and fixture variant count diverge.

- [ ] **Step 2: Run Rust contract test and verify RED**

```sh
pnpm rust:test --lib checked_in_node_system_contracts_match_rust -- --nocapture
```

Expected: FAIL because `project-events.json` and `execution-wire.json` do not exist in the contract map.

- [ ] **Step 3: Add failing TypeScript parser tests**

For each fixture variant assert parsing succeeds. For each envelope mutate one property at a time:

```ts
expect(() => parseRunEvent({ ...valid, extra: true })).toThrow();
expect(() => parseRunEvent({ ...valid, kind: { type: 'unknown' } })).toThrow();
expect(() => parseExecuteGraphResultDto({ runId: 41 })).toThrow();
expect(() => parseExecutionDemandDto({ type: 'outputs', outputs: [], includeDefaultResults: false })).not.toThrow();
```

- [ ] **Step 4: Run TypeScript tests and verify RED**

```sh
pnpm test src/shared/types/dto/runEventParser.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
```

Expected: FAIL because fixtures/parsers are absent.

- [ ] **Step 5: Generate exact Rust fixtures**

Extend `contracts()` with `project-events.json` and `execution-wire.json`. Use production Serde encoders only; do not hand-author Rust expected output and fixture independently. Update protected fixture hashes only through the repository's existing contract update workflow and review the JSON diff.

- [ ] **Step 6: Implement strict TypeScript parsers**

Use exhaustive `switch (kind.type)` and exact-key guards. Decimal opaque IDs remain strings matching `/^(0|[1-9]\d*)$/`; graph revisions and resource versions remain strings where the Rust wire uses strings. Validate `GraphOutputRefDto` through the existing editor projection port-address parser or extract one shared strict helper.

Wire `parseExecutionDemandDto` before constructing the invoke payload, `parseRunEvent` into the execution channel before `onEvent` and terminal-drain observation, and `parseExecuteGraphResultDto` immediately after invoke.

- [ ] **Step 7: Run contract tests and verify GREEN**

Run Steps 2 and 4. Expected: PASS and checked-in fixtures byte-match Rust production serialization.

- [ ] **Step 8: Review checkpoint**

Confirm all `RunEventKindDto` variants are represented and no TypeScript parser uses `as RunEvent` on raw IPC values.

---

### Task 7: Enforce architecture policy, run broad verification, and update TODO

**Files:**
- Modify: `src/services/project/projectFilesystemContract.test.ts`

- Modify: `TODO.md`
- Review: all files changed in Tasks 1–6

**Interfaces:**
- Produces a source audit mapping every active-project command to required `projectInstanceId`.
- Marks only the three completed Batch A TODO entries as done.

- [ ] **Step 1: Add failing command-policy audit**

Extend the identity allowlist with:

```ts
const activeProjectCommandIdentityFields = {
  mutate_graph_document: 'projectInstanceId',
  update_function_signature: 'projectInstanceId',
  hydrate_editor_graph: 'projectInstanceId',
  get_project_history_status: 'projectInstanceId',
  undo_graph_document: 'projectInstanceId',
  redo_graph_document: 'projectInstanceId',
  execute_graph_document: 'projectInstanceId',
} as const;
```

Audit both service invoke payloads and Rust Tauri command signatures. Explicitly list bootstrap/global/capability commands as exemptions so an unclassified command fails the audit.

- [ ] **Step 2: Run policy audit and verify RED if any path remains unclassified**

```sh
pnpm test src/services/project/projectFilesystemContract.test.ts
```

Expected before final cleanup: FAIL listing any missing service or Rust identity parameter.

- [ ] **Step 3: Fix remaining classification/signature gaps**

Do not weaken the audit or add wildcard exemptions. Update the actual command/service path, then rerun until PASS.

- [ ] **Step 4: Run focused frontend matrix**

```sh
pnpm typecheck
pnpm test src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/features/application/editorMutation/editorMutationCoordinator.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/editorMutation/historyCoordinator.test.ts src/features/application/editorProjection/graphProjectionCoordinator.test.ts src/features/application/editor/requestPinPreview.test.ts src/services/nodeSystem/graphMutationService.test.ts src/services/nodeSystem/functionMutationService.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/services/nodeSystem/historyService.test.ts src/services/project/projectService.execution.test.ts src/shared/types/dto/runEventParser.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/services/project/projectFilesystemContract.test.ts
```

Expected: typecheck exits 0; all selected test files PASS.

- [ ] **Step 5: Run focused Rust matrix serially**

Run each new test filter with `pnpm rust:test --lib <filter> -- --test-threads=1 --nocapture`, then run:

```sh
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all focused tests PASS; fmt/check exit 0. Existing warnings may remain but no new warning is introduced.

- [ ] **Step 6: Run broader verification**

```sh
pnpm test
pnpm rust:test --lib -- --test-threads=1
pnpm verify
```

Expected: frontend suite passes. On Windows, separately record the three known reparse-point tests if they fail with OS error 1314; do not relabel assertion failures as environment failures. If `pnpm verify` stops on an artifact/permission issue, retain fresh focused evidence and report the exact command/error.

- [ ] **Step 7: Request independent review**

Use the code-review workflow to check:

- no stale command can mutate a replacement project;
- event/direct identity is authority-sourced;
- no optional compatibility path exists;
- parsers cover every Rust variant;
- stale paths have zero effects and no misleading toast/recovery.

Fix all Critical and Important findings with new failing tests before proceeding.

- [ ] **Step 8: Update TODO with fresh evidence**

In `TODO.md` under `2026.08.06`, mark only these completed items `[x]`:

- GraphDelta project-event wire identity;
- active-project node command lifecycle identity;
- execution/project-event Rust↔TS golden coverage.

Add exact verification commands/results. Leave every Batch B–D omission unchecked.

- [ ] **Step 9: Final hygiene check**

```sh
git diff --check
git --no-optional-locks status --short
git --no-pager diff --stat
```

Expected: no whitespace errors, no staged changes, no unintended generated/build files, and only planned/user-existing modifications.
