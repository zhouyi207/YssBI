# Rust Suite Contract Stabilization Design

## Goal

Repair the fifteen deterministic stale, invalid, or brittle Rust tests remaining after the four production architecture blockers were fixed, and close the resource-plan preflight gap discovered during audit.

## Scope

This slice covers only the individually diagnosed failures from the single known-red complete Rust run:

- three source-text architecture assertions;
- six invalid or stale fixtures/contracts;
- six brittle numerical, identity, call-count, or error-display assertions;
- one newly identified production gap in unsupported resource-access preflight.

The complete Rust suite is not rerun until every focused task is reviewed and green and the user explicitly authorizes another complete-suite attempt.

## Source-contract tests

Delete these three source-text tests from `commands/command_node_system.rs`:

- committed resource completion source is total/state-independent;
- projection capture lock order is activation-compatible;
- projection capture rejects mixed activation generation.

Do not replace them with updated string or AST matching. Their supported contracts are already covered by behavioral tests for:

- committed completion returning a canonical result or explicit incomplete projection;
- captured database metadata surviving post-commit changes;
- stale authority refusing rebinding;
- activation overlap completing without deadlock;
- projection environment containing one coherent project generation;
- activation-publication panic restoring an even generation and complete session.

The supported completion contract is deliberately narrower than absolute state independence: committed completion does not recapture authoritative domain/database metadata, but it may still use the live compile coordinator and authority gate.

## Fixture repairs

Repair tests at authoritative entry points rather than weakening expected behavior:

- activate replacement state before creating project-index fixture data;
- use canonical function-resource creation for function body change coverage;
- use an active-project graph helper for scope/shell compiler diagnostics;
- create authoritative variables through `ProjectState::add_variable` rather than direct map insertion;
- derive relational pushdown hints from operators rather than hand-building inconsistent compiled metadata;
- isolate scheduler validation-error mapping with a purpose-built test provider.

Do not globally change inactive test helpers, because many tests intentionally exercise inactive lifecycle behavior.

## Resource preflight production contract

`ProjectResourceProvider::validate_plan` must reject unsupported access modes before any resource acquisition. Exclusive project-variable access remains supported. Unsupported exclusive access for database or other read-only resources returns `ResourceErrorKind::UnsupportedAccess`, which `RunExecutor` classifies as `RunError::InvalidPlan` / `RunErrorCode::InvalidPlan`.

Add a focused provider regression proving validation fails before acquisition. Keep the scheduler mapping test independent by using a deterministic provider that returns the validation error directly.

## Brittle assertion repairs

### Statistics

Preserve production `f64` output. Assert list shape, decimal element types, length, and numerical values using scaled tolerance:

```text
abs(actual - expected) <= 1e-12 * max(abs(expected), 1.0)
```

### Determinism snapshots

Normalize only `ExecutionPlan.provenance.compile_id` to zero in the shared debug snapshot helper. Preserve graph path, session, basis, revisions, resources, and all semantic plan content.

### Capture retries

Keep authoritative result/revision assertions. Replace exact internal capture count `2` with `>= 2`, because the publication authority gate performs an additional legitimate capture. Extra captures are a performance concern, not the semantic retry contract.

### Recovery errors

Change the test-only `load_graph` helper to return typed `ProjectFilesystemError`. Assert `.code() == "project_recovery_required"` and `recovery_required() == true`. Do not require human-readable `Display` text to contain the machine code.

## Error and behavior preservation

- Production numerical output is unchanged.
- Production compile IDs remain unique and monotonic.
- Capture/publication behavior is unchanged.
- Recovery display strings and IPC mappings are unchanged.
- Active-project authority remains mandatory.
- Function shell and graph insertion continue through existing ProjectState paths.
- Relational pushdown validation remains strict.
- Resource validation is strengthened only for access modes that acquisition cannot legally satisfy.

## Task boundaries

1. Remove source-text tests and verify the replacement behavioral coverage.
2. Repair active-project and canonical-resource fixtures.
3. Repair relational/resource validation fixtures and implement provider preflight.
4. Repair numerical/determinism/capture/recovery assertions.
5. Run all fifteen exact regressions, relevant focused owner suites, Rust gates, and final independent review.

Each task receives an independent review and fresh controller verification. Update `TODO.md` immediately after each reviewed task.

## Verification

Use serial focused Rust commands with `CARGO_BUILD_JOBS=1` and `--test-threads=1`. Required final gates are:

- `pnpm rust:check`;
- `pnpm rust:fmt:check`;
- `git diff --check`.

Do not rerun the complete Rust suite within this slice without explicit user authorization. Preserve the known baseline warnings and unrelated dirty work.
