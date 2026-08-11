# Frontend Test Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove only frontend tests and assertions that have no independent regression value.

**Architecture:** This is a test-only cleanup. Work is grouped by test responsibility so each batch can be reviewed and validated independently; production code and meaningful architecture, IPC, parser, lifecycle, interaction, accessibility, and layout coverage remain unchanged.

**Tech Stack:** TypeScript, React, Vitest, pnpm

## Global Constraints

- Do not modify production code.
- Preserve unrelated working-tree changes.
- Delete only fixture self-tests, exact duplicates, constant/type restatements, native Promise passthrough tests, unreachable IPC inputs, and structurally tautological assertions.
- Preserve architecture audits, Rust-to-TypeScript wire contracts, IPC payload contracts, opaque resource identity, concurrency, lifecycle, error, interaction, accessibility, scrolling, canvas, and flex-layout coverage.
- Do not create Git commits unless explicitly requested.

---

### Task 1: Remove DTO and shared-domain self-tests

**Files:**
- Delete: `src/shared/types/dto/runEvent.correlation.test.ts`
- Modify: `src/shared/types/dto/executionDemand.test.ts`
- Modify: `src/shared/types/dto/runEventParser.test.ts`
- Modify: `src/shared/types/domain/pinVisual.test.ts`
- Modify: `src/shared/utils/pinCompatibility.test.ts`

**Interfaces:**
- Consumes: Existing production parsers and compatibility functions.
- Produces: The same runtime and compile-time coverage without fixture self-tests or exact duplicate cases.

- [ ] **Step 1: Delete the correlation fixture self-test**

Delete `src/shared/types/dto/runEvent.correlation.test.ts`; it constructs both compared objects locally and never invokes production code.

- [ ] **Step 2: Remove runtime object self-tests from execution demand coverage**

From `src/shared/types/dto/executionDemand.test.ts`, delete these two test cases:

```text
freezes default, declared, instance, empty, and duplicate-order wire shapes
freezes stable outputReady identity without compiler-local fields
```

Remove imports used only by those cases: `DEFAULT_EXECUTION_DEMAND` and `RunEventKind`. Retain `ExecutionDemandDto`, `parseExecutionDemandDto`, the strict parser test, and every `@ts-expect-error` assertion.

- [ ] **Step 3: Remove fixture-only assertions from run event parser coverage**

From `src/shared/types/dto/runEventParser.test.ts`:

```text
Delete: freezes ordinary null and preview numeric output generations
Delete only: expect(operations.length).toBe(3)
```

Retain the operation mutation loop and all calls through `parseRunEvent`.

- [ ] **Step 4: Remove exact duplicate domain cases**

From `src/shared/types/domain/pinVisual.test.ts`, delete:

```text
derives container overlay from dataType only
```

From `src/shared/utils/pinCompatibility.test.ts`, keep one of the two cases that creates an output object pin without structured `dataType` and expects `buildPinDataType` to throw `missing structured dataType`; delete the duplicate case. Also delete these wrapper cases because the same inputs and branches are covered directly through `canConnectPins`:

```text
rejects same-direction and same-node pairs
highlights concrete Struct model outputs for Model family inputs
```

- [ ] **Step 5: Validate shared test cleanup**

Run:

```text
pnpm exec vitest run src/shared/types/dto/executionDemand.test.ts src/shared/types/dto/runEventParser.test.ts src/shared/types/domain/pinVisual.test.ts src/shared/utils/pinCompatibility.test.ts
```

Expected: all retained tests pass.

---

### Task 2: Remove duplicated application and editor tests

**Files:**
- Modify: `src/features/application/editorMutation/editorMutation.test.ts`
- Modify: `src/features/application/editor/editorUnavailableRouting.test.tsx`
- Modify: `src/features/application/presentation/parsePresentationWindowQuery.test.ts`
- Modify: `src/features/application/presentation/loadPresentationWindow.test.ts`
- Modify: `src/features/application/graphDocument/graphDocumentActions.test.ts`

**Interfaces:**
- Consumes: Dedicated service tests and existing consumer behavior tests.
- Produces: Application tests focused on coordination behavior instead of duplicate service or constant coverage.

- [ ] **Step 1: Delete the obsolete service test block from editor mutation tests**

Delete the complete `mutation and history services` describe block from `src/features/application/editorMutation/editorMutation.test.ts`, including these seven tests:

```text
sends the canonical declared port mutation JSON
sends one exact atomic parameter mutation without numeric coercion
sends the canonical dynamic instance mutation JSON
models canonical resource delta JSON with camelCase correlation fields
models worksheet content through the common resource delta contract
sends revisioned function signature requests through the thin node-system service
keeps history services invoke-only and sends project identity, locale, and request
```

Remove imports used only by the deleted block: `FunctionMutationService`, `HistoryService`, `MutationRequestDto`, and `ResourceMutationResultDto`. Retain `GraphMutationService` if it is used by the `executeEditorMutation` tests below the deleted block.

- [ ] **Step 2: Remove the capability constant restatement**

From `src/features/application/editor/editorUnavailableRouting.test.tsx`, delete:

```text
enables Catalog descriptors and documentation while duplicate and paste stay disabled
```

Remove `EDITOR_MUTATION_CAPABILITIES` if no retained test uses it. Preserve all consumer behavior tests for descriptors, documentation, duplicate, and paste.

- [ ] **Step 3: Remove duplicated query decoding coverage**

From `src/features/application/presentation/parsePresentationWindowQuery.test.ts`, delete:

```text
decodes encoded slashes in sourceId
```

The preceding `URLSearchParams` case already produces and parses the same `%2F` input.

- [ ] **Step 4: Remove structurally tautological assertions**

From `src/features/application/presentation/loadPresentationWindow.test.ts`, delete only:

```ts
expect(SourceService.getPinDescriptor).not.toHaveBeenCalled();
```

Remove the mocked member only if no retained test uses it.

From `src/features/application/graphDocument/graphDocumentActions.test.ts`, delete only:

```ts
expect('updateCallFunctionTarget' in graphDocumentActions).toBe(false);
```

Keep the coordinator delegation and store-authority assertions.

- [ ] **Step 5: Validate application cleanup**

Run the five modified test files with `pnpm exec vitest run`. Expected: all retained tests pass.

---

### Task 3: Remove redundant service tests and unreachable parser inputs

**Files:**
- Modify: `src/services/nodeSystem/functionMutationService.test.ts`
- Modify: `src/services/nodeSystem/graphProjectionService.test.ts`
- Modify: `src/services/worksheet/worksheetService.test.ts`
- Modify: `src/services/project/projectService.test.ts`
- Modify: `src/services/nodeSystem/catalogService.test.ts`

**Interfaces:**
- Consumes: Thin Tauri service wrappers and strict wire parsers.
- Produces: Service coverage for commands, payloads, responses, and malformed data without testing native Promise behavior or duplicate positive fixtures.

- [ ] **Step 1: Remove native Promise rejection passthrough tests**

Delete:

```text
src/services/nodeSystem/functionMutationService.test.ts
  preserves project identity when the command rejects

src/services/nodeSystem/graphProjectionService.test.ts
  preserves project identity when hydrate rejects

src/services/worksheet/worksheetService.test.ts
  preserves and formats the exact %s worksheet error contract
```

For the worksheet parameterized case, delete the complete six-row error matrix and remove imports used only for `formatErrorMessage` or those rows.

- [ ] **Step 2: Remove unreachable JavaScript evaluation-order tests**

From `src/services/project/projectService.test.ts`, delete:

```text
rejects unknown keys before evaluating graph and database values
rejects inherited required keys before evaluating invalid rows
```

Keep exact-key, own-property, malformed-row, and strict parser tests that use JSON-representable values.

- [ ] **Step 3: Remove duplicate manual Catalog positive fixtures**

From `src/services/nodeSystem/catalogService.test.ts`, delete:

```text
requests the localized catalog with the project identity and locale
accepts an exact parameterized-static descriptor without reconstructing it
accepts the exact resource-bound item metadata and descriptor
```

Retain the Rust-generated localized Catalog fixture test and every malformed or metadata/descriptor mismatch negative test.

- [ ] **Step 4: Validate service cleanup**

Run all five modified service test files with `pnpm exec vitest run`. Expected: command/payload, parser, malformed response, and negative descriptor tests pass.

---

### Task 4: Remove core, domain, and view constant restatements

**Files:**
- Modify: `src/features/domain/nodeCatalog/identity.test.ts`
- Modify: `src/features/core/dataStore/graphProjectionStore.test.ts`
- Modify: `src/features/core/layout/workbenchPanelSizing.test.ts`
- Modify: `src/features/core/layout/panelPartModel.test.ts`
- Modify: `src/views/BayesView/components/BayesPanels.test.ts`
- Modify: `src/views/EditorView/Layout/Detail/nodeDocumentation.test.ts`
- Modify: `src/views/EditorView/Layout/Detail/node/parameterEditors/RelationalParameterEditors.test.tsx`
- Modify: `src/views/EditorView/Layout/Detail/observability/GraphTraceDetails.i18n.test.ts`
- Modify: `src/views/EditorView/Layout/NodePalette.test.tsx`
- Modify: `src/views/EditorView/Layout/Detail/panels/NodeDetailPanel.test.ts`

**Interfaces:**
- Consumes: Existing runtime behavior and architecture contract coverage.
- Produces: Tests that validate consumers and behavior rather than exact constants, prose, internal React keys, or weak source-string checks.

- [ ] **Step 1: Remove node identity constant and duplicate legacy assertions**

From `src/features/domain/nodeCatalog/identity.test.ts`, delete:

```text
uses the Rust-defined built-in node type IDs
```

Within `classifies stable IDs and rejects every legacy display identity`, delete the four individual legacy display identity assertions immediately before `legacyIdentities`; retain the loop, which covers all three classifiers for every legacy identity.

- [ ] **Step 2: Remove type and derived-constant restatements**

From `src/features/core/dataStore/graphProjectionStore.test.ts`, remove the `expectTypeOf` assertions that restate every `GraphEntityBucket` field. Remove imports used only by those assertions. Preserve the runtime selector assertion and metadata storage tests.

From `src/features/core/layout/workbenchPanelSizing.test.ts`, delete:

```ts
expect(PANEL_MAX_VIEWPORT_RATIO).toBe(0.8);
```

Remove the constant import if unused; preserve the `1000` viewport to `800` maximum behavior assertion.

From `src/features/core/layout/panelPartModel.test.ts`, delete direct assertions that `PANEL_VIEW_SPECS.output.implemented` and `.terminal.implemented` are `false`. Remove the import if unused; preserve default-view and fallback behavior assertions.

- [ ] **Step 3: Remove exact text and constant self-equality assertions**

From `src/views/BayesView/components/BayesPanels.test.ts`, delete only:

```ts
expect(draft.formulaText).toBe(DEFAULT_BAYES_FORMULA);
```

Remove `DEFAULT_BAYES_FORMULA` if unused; preserve symbol derivation and response expression assertions.

Delete the complete `node documentation markdown samples` describe block from `src/views/EditorView/Layout/Detail/nodeDocumentation.test.ts`.

Delete the complete `relational parameter editor localization` describe block from `src/views/EditorView/Layout/Detail/node/parameterEditors/RelationalParameterEditors.test.tsx`; remove direct `enUS` and `zhCN` imports if unused.

From `src/views/EditorView/Layout/Detail/observability/GraphTraceDetails.i18n.test.ts`, delete exact translated-value assertions and retain only the English/Chinese key parity assertion.

- [ ] **Step 4: Remove behavior-duplicated internal implementation checks**

From `src/views/EditorView/Layout/NodePalette.test.tsx`, delete:

```text
keys same-type resources by exact descriptor tuple
```

Remove `nodePaletteItemKey` if imported only by that test. Preserve real rendering tests that distinguish same-type resources through resource path, refresh, search, and locale changes.

From `src/views/EditorView/Layout/Detail/panels/NodeDetailPanel.test.ts`, delete:

```text
does not read Call Function legacy fields or legacy catalogs
```

Remove `readFileSync` if unused. Preserve graph-path-based selection behavior; legacy identity is already protected by dedicated architecture contracts.

- [ ] **Step 5: Validate core, domain, and view cleanup**

Run all ten modified files with `pnpm exec vitest run`. Expected: retained runtime, rendering, identity, layout, localization parity, and behavior tests pass.

---

### Task 5: Run complete frontend verification

**Files:**
- Verify all modified and deleted frontend tests.
- Verify: `docs/superpowers/specs/2026-08-11-frontend-test-cleanup-design.md`
- Verify: `docs/superpowers/plans/2026-08-11-frontend-test-cleanup.md`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: Evidence that the cleanup preserves frontend correctness.

- [ ] **Step 1: Run TypeScript validation**

Run:

```text
pnpm typecheck
```

Expected: exit code 0 with no TypeScript errors.

- [ ] **Step 2: Run the complete frontend test suite**

Run:

```text
pnpm exec vitest run
```

Expected: all frontend test files pass. If an unrelated pre-existing failure appears, record its file and error separately; do not modify unrelated production behavior.

- [ ] **Step 3: Check the working-tree diff**

Run:

```text
git diff --check
```

Expected: exit code 0. Existing line-ending warnings in unrelated Rust files may remain, but there must be no whitespace errors.

- [ ] **Step 4: Review preservation constraints**

Confirm the final diff changes only test files and the approved design/plan documents, leaves production code untouched, and does not remove architecture audits, Rust fixtures, parser negative cases, IPC payload contracts, lifecycle coverage, interaction coverage, accessibility checks, or layout regression tests.
