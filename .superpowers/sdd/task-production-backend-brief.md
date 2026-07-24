# Task: Switch backend production authority to node_system

Implement the backend half of the irreversible production cut described by `docs/plan/node-architecture.md`, especially sections 1, 10-13, 18, 20, 21 and 23.

## Requirements

- No compatibility adapters, old ID aliases, dual writes, or fallback readers.
- `ProjectState.project_data` remains the project authority, but its graph resources must use normalized `node_system::document::GraphDocument` and resource documents, never `graph::GraphInstance`.
- `ProjectStore` must own the immutable new built-in `node_system::registry::NodeRegistry`, production kernels/resources/function plans, not the old mutable Registry.
- Replace graph persistence with a new schema version that serializes graph resource metadata + normalized GraphDocument. Fixed ports are not persisted. Old graph files are rejected, not converted.
- All graph mutations use revisioned `MutationRequest`/document patches/Rust `ProjectHistory`; remove frontend snapshot rebuild and old pin-ID mutation paths.
- Graph load/hydrate returns localized `EditorGraphProjectionDto`, not old GraphInstanceDTO/PinInstanceDTO.
- Expose localized catalog command and revisioned graph mutation/projection commands through thin Tauri commands.
- Execution snapshots document/resources under short locks, compiles via GraphCompiler, rejects blocking/stale plans, runs via new RunExecutor, and uses new runtime events/result store.
- Do not query Registry, display roles, i18n or editor state during Run.
- Remove backend production registrations and module references for obsolete graph mutation/schema/execution paths. Delete old modules only where no remaining non-node domain dependency requires them; moving shared value types to proper ownership is allowed.
- Preserve unrelated database/project/worksheet behavior.
- Keep locks short; no I/O, compilation, event emit or execution under project locks.
- Update focused Rust tests for project IO round-trip, fixed ports absent, mutation revision conflict/history, projection hydrate, blocking execution refusal, and resource cleanup.

## Validation

Run sequentially with one build job:

1. `CARGO_BUILD_JOBS=1 pnpm rust:check`
2. focused project IO test
3. focused project graph mutation test
4. focused execution test
5. `git diff --check`

Do not run tests concurrently. Do not commit.
