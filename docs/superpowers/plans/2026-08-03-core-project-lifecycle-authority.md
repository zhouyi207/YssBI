# Core Project Lifecycle Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish one dependency-safe Core owner for frontend project lifecycle identity, migrate all identity consumers, and enforce service boundaries with an AST-resolved architecture contract.

**Architecture:** A framework-light module under `features/core/projectLifecycle` owns active project instance and epoch only. Publication and graph projection coordinators consume this authority but do not own or route identity for each other. A TypeScript AST audit resolves alias and relative imports from production services and rejects every path into features or views.

**Tech Stack:** TypeScript, Vitest, TypeScript compiler API, React/Zustand integration tests, pnpm.

## Global Constraints

- Work directly on `shadcn`; do not create a worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- Keep publication revision, recovery, and deduplication in `projectPublicationCoordinator`.
- Do not change Resource Catalog DTOs, database recovery wire, backend paths, revisions, or event families.
- Do not keep compatibility shims or duplicate lifecycle owners.
- Core lifecycle authority must not import application, services, views, Tauri, or domain stores.
- Services must not import features or views; views must not invoke Tauri.
- Use RED-GREEN TDD and focused tests.
- Update `.superpowers/sdd/2026-08-03-core-project-lifecycle-authority/progress.md` and `TODO.md` after every independently reviewed task.

---

## File Structure

### Core authority and migration

- Create `src/features/core/projectLifecycle/projectLifecycleAuthority.ts`: sole lifecycle state and capture/assert API.
- Create `src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts`: direct lifecycle semantics.
- Modify `src/features/application/editorMutation/projectPublicationCoordinator.ts`: delegate activation/reset identity ownership to Core.
- Modify every current import of `src/features/application/projectIdentity.ts`, including application coordinators, Core stores/sessions/event handlers, and focused tests.
- Delete `src/features/application/projectIdentity.ts` after migration; no re-export.

### Architecture contracts

- Modify `src/services/project/projectFilesystemContract.test.ts`: replace regex-only service import audit with AST/resolution coverage and update identity-owner assertions.
- Create a small test-local helper in the same test file or `src/tests/helpers/architectureImportAnalysis.ts` only if reuse materially improves clarity. Production runtime must not depend on TypeScript compiler parsing.
- Add a scoped lifecycle boundary contract covering Core authority imports and deleted facade paths.

---

### Task 1: Create Core lifecycle authority and migrate consumers

**Files:**

- Create: `src/features/core/projectLifecycle/projectLifecycleAuthority.ts`
- Create: `src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts`
- Modify: `src/features/application/editorMutation/projectPublicationCoordinator.ts`
- Modify: `src/features/application/projectCommandContext.ts`
- Modify: `src/features/application/editorProjection/graphProjectionCoordinator.ts`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.ts`
- Modify: `src/features/application/editor/graphDocumentUnload.ts`
- Modify: `src/features/application/nodeCatalog/useLocalizedNodeCatalog.ts`
- Modify: `src/features/application/projectLifecycleReceiptDependencies.ts`
- Modify: `src/features/core/dataStore/projectIOStore.ts`
- Modify: `src/features/core/dataStore/projectSession.ts`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.ts`
- Modify: `src/features/core/sync/handlers/ResourceEventHandler.ts`
- Modify: focused tests importing or mocking the old facade
- Delete: `src/features/application/projectIdentity.ts`

**Interfaces:**

- Produces one Core API equivalent to:

```ts
export interface ProjectLifecycleSnapshot {
  readonly projectInstanceId: string;
  readonly projectEpoch: number;
}

export function startProjectLifecycle(projectInstanceId: string): void;
export function clearProjectLifecycle(): void;
export function captureProjectLifecycle(): ProjectLifecycleSnapshot;
export function isProjectLifecycleCurrent(snapshot: ProjectLifecycleSnapshot): boolean;
export function assertProjectLifecycleCurrent(snapshot: ProjectLifecycleSnapshot): void;
```

Names may preserve existing `ProjectIdentitySnapshot` terminology if that avoids unnecessary call-site churn. There must be one mutable owner only.

- Publication coordinator may expose publication-specific lifecycle snapshots containing publication revision, but obtains project instance/epoch from Core authority rather than owning parallel identity state.

- [ ] **Step 1: Write direct Core authority RED tests**

Cover no active project, first activation, replacement invalidation, clear/reset invalidation, immutable capture, stale assertion compatibility, and repeated activation semantics.

- [ ] **Step 2: Run RED direct tests**

```sh
pnpm test src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts
```

Confirm failure is due to the absent Core authority module/API.

- [ ] **Step 3: Implement the minimal Core authority**

Keep state module-private. Epoch must monotonically invalidate all snapshots on replacement and clear. Do not import coordinators, stores, services, views, or Tauri.

- [ ] **Step 4: Add RED migration/cycle tests**

Update focused lifecycle tests to import Core authority and assert project replacement still rejects stale graph hydration, publications, events, and revisioned commands. Add a temporary assertion that the old application facade is absent and current Core stores do not import it.

- [ ] **Step 5: Delegate publication identity to Core**

Refactor `projectPublicationCoordinator` activation/reset/capture methods so publication-specific state remains local while base identity and epoch come from Core. Preserve waiter cancellation, recovery generation, publication revision, and operation settlement ordering.

- [ ] **Step 6: Migrate every consumer and delete the old facade**

Update all imports discovered by the repository search. Core modules must import only the Core authority. Delete `src/features/application/projectIdentity.ts` without a shim.

- [ ] **Step 7: Run focused GREEN tests**

At minimum run:

```sh
pnpm test src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts src/features/application/projectCommandContext.test.ts src/features/application/dataManagement/databaseMutation.test.ts src/features/application/dataManagement/variableActions.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/editorProjection/graphProjectionCoordinator.test.ts src/features/application/editor/graphSessionLifecycle.test.ts src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
pnpm typecheck
git diff --check
```



- [ ] **Step 8: Independent review and publication**

Reviewer must verify one mutable lifecycle owner, no identity cycle, no Core→application identity dependency, stale behavior preservation, and no publication/recovery ownership leakage. After approval, update the ledger and `TODO.md` Phase 3.

---

### Task 2: Replace regex boundary checks with AST-resolved audits

**Files:**

- Modify: `src/services/project/projectFilesystemContract.test.ts`
- Optional create: `src/tests/helpers/architectureImportAnalysis.ts`
- Test: mutation fixtures inside the architecture contract test

**Interfaces:**

- The audit enumerates production `src/services/**/*.{ts,tsx}` files.
- It parses each source with the TypeScript compiler API.
- It normalizes `@/x` to `src/x` and resolves relative specifiers from the importing file.
- It rejects resolved paths under `src/features` or `src/views`.

- [ ] **Step 1: Add RED import-form mutation fixtures**

Each fixture must be parsed by the same analyzer used for production sources and independently prove rejection of:

```ts
import value from '@/features/core/example';
import '@/features/core/example';
await import('@/views/example');
const value = require('../features/example');
import value = require('@/features/application/example');
import value from '../../features/core/example';
```

Use relative examples whose resolved target actually lands under `src/features` or `src/views` from the fixture importer path.

- [ ] **Step 2: Run RED architecture test**

```sh
pnpm test src/services/project/projectFilesystemContract.test.ts
```

Confirm side-effect, require/import-assignment, and relative cases expose the current regex audit gaps.

- [ ] **Step 3: Implement AST import extraction and path resolution**

Handle static bindings, side-effect imports, dynamic import calls, CommonJS require calls, and TypeScript import-equals declarations. Ignore non-literal runtime expressions rather than inventing a path; production code containing such dynamic service module resolution should be reported separately by a focused unsupported-form check if present.

- [ ] **Step 4: Add lifecycle boundary assertions**

Assert:

```text
old application identity facade does not exist
Core lifecycle authority imports no application/services/views modules
publication and graph projection coordinators import Core authority
no service production file resolves into features/views
```

Do not globally ban application modules from importing services.

- [ ] **Step 5: Run GREEN architecture and identity tests**

```sh
pnpm test src/services/project/projectFilesystemContract.test.ts src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts src/features/application/projectCommandContext.test.ts
pnpm typecheck
git diff --check
```

- [ ] **Step 6: Independent review and publication**

Reviewer must inspect every supported syntax fixture, alias/relative normalization, production file enumeration, negative controls, and audit scope. After approval, update the ledger and `TODO.md` Phase 3.

---

### Task 3: Final verification and whole-slice review

**Files:**

- Modify: `.superpowers/sdd/2026-08-03-core-project-lifecycle-authority/progress.md`
- Modify: `TODO.md`

- [ ] **Step 1: Run the complete lifecycle/identity focused set**

Include all Task 1 and Task 2 explicit frontend files, publication/recovery/event handler tests, graph hydration lifecycle tests, and the revisioned command tests from the preceding plan.

- [ ] **Step 2: Run the established Resource Catalog aggregate**

Run this exact aggregate without an extra `--` separator:

```sh
pnpm test src/services/database/databaseService.test.ts src/features/application/dataManagement/useDatabaseManagement.test.tsx src/features/application/resource/resourceActions.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/core/sync/utils/resourceMutationWireValidator.test.ts src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/features/application/editorMutation/projectPublicationProductionStores.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/features/domain/nodeCatalog/search.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/views/EditorView/Layout/NodePalette.test.tsx src/services/nodeSystem/catalogService.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/dataManagement/useNodeManagement.test.tsx src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/views/EditorView/Layout/NodeDocumentationModal.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/services/nodeSystem/nodeCatalogArchitectureContract.test.ts src/features/application/projectCommandContext.test.ts src/features/application/dataManagement/databaseMutation.test.ts src/features/application/dataManagement/variableActions.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts src/services/project/projectFilesystemContract.test.ts
```

- [ ] **Step 3: Run the established focused Rust matrix serially**

Run all Resource Catalog Tasks 1–4/7 filters, ProjectIndex declaration/coherence filters, database library tests, and the `database_test` integration target with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.

- [ ] **Step 4: Run final gates**

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 5: Dispatch independent whole-slice review**

Reviewer must verify:

```text
one Core lifecycle owner
no application/core identity cycle
no Core identity dependency on application/services/views
no services import features/views through supported syntax
unchanged stale command/publication/graph/event behavior
unchanged Resource Catalog watermark, database recovery, and opaque paths
```

It must also adjudicate the deferred graph-session fixture concern and confirm the database integration target remains green.

- [ ] **Step 6: Publish only with fresh controller evidence**

Append exact counts and review verdict to the ledger. Raise `TODO.md` only when no Critical or Important finding remains.
