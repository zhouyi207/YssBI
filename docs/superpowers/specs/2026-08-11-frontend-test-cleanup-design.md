# Frontend Test Cleanup Design

## Goal

Remove only frontend tests that have no independent regression value, reducing maintenance noise without weakening behavioral, architectural, IPC, cross-language, interaction, accessibility, layout, lifecycle, or concurrency coverage.

## Scope

The cleanup covers tests under `src/`. Production code is out of scope. Existing unrelated working-tree changes must remain untouched.

A test or assertion is eligible for deletion only when review confirms at least one of these conditions:

- It does not execute production code and only checks an object or fixture created by the test itself.
- Another test exercises the same production path with the same input class and equal or stronger assertions.
- It directly restates a static constant, exact translation text, or TypeScript interface without checking a consumer or runtime boundary.
- It only demonstrates native Promise rejection propagation through a wrapper with no error handling.
- It constrains evaluation order using getter or prototype inputs that cannot cross the Tauri JSON/Serde IPC boundary.
- Its result is structurally guaranteed by the test setup and cannot expose a production regression.

## Preservation Rules

Keep tests that protect any of the following, even when their implementation is simple or their assertions are concrete:

- Dependency direction and architecture boundaries.
- Rust-generated fixtures passing frontend wire parsers.
- Tauri command names, payload casing, project identity, operation identity, and revisions.
- Opaque resource paths and backend-issued descriptors.
- State-machine transitions, stale-result protection, concurrency, lifecycle, and recovery.
- Error parsing and malformed DTO rejection.
- User interaction, accessibility, selection, scrolling, canvas sizing, and required flex layout contracts.
- User-visible policy mappings that are not duplicated elsewhere.

## Change Strategy

Delete only high-confidence candidates already identified by the full frontend test audit. Prefer removing a redundant assertion or test case over deleting an entire file when useful coverage remains. Do not replace deleted tests with equivalent snapshots or implementation-detail assertions. Clean up imports and helpers that become unused as a direct result.

## Validation

After cleanup:

1. Run focused Vitest coverage for every modified test area.
2. Run `pnpm typecheck` so retained compile-time assertions and imports remain valid.
3. Run the complete frontend Vitest suite.
4. Run `git diff --check`.

Any failure caused by the cleanup must be fixed without restoring tests that meet the deletion criteria. Pre-existing unrelated failures must be reported separately.
