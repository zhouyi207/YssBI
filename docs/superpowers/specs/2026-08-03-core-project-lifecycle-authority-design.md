# Core Project Lifecycle Authority Design

## Status

Approved design for removing the remaining lifecycle identity dependency cycle and strengthening service-boundary audits.

## Constraints

- Work directly on `shadcn`; do not create a worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- Keep Rust authoritative for persisted project/resource state and revisions.
- Keep publication revision, recovery, and deduplication in the application publication coordinator.
- Do not change Resource Catalog watermarks, database recovery wire, or mutation event families.
- Do not keep compatibility shims or duplicate identity owners.
- Frontend services must not import features or views; views must not invoke Tauri directly.
- Use focused RED-GREEN tests and update `TODO.md` after every independently reviewed task.

## Problem

Moving `projectIdentity` from services into application removed a direct `services → features` violation but retained an architecture cycle:

```text
projectIdentity
  → projectPublicationCoordinator
  → graphProjectionCoordinator
  → projectIdentity
```

Core stores and event handlers also import the application-level identity facade. This reverses the intended dependency direction: project lifecycle identity is shared infrastructure, while publication and graph projection are application workflows.

The current service-boundary audit also detects only selected alias import forms and can miss side-effect imports, CommonJS `require`, TypeScript import assignments, and relative paths that resolve into `src/features` or `src/views`.

## 1. Core lifecycle authority

Create `src/features/core/projectLifecycle/projectLifecycleAuthority.ts` as the sole owner of frontend project lifecycle identity.

It owns only:

- active `projectInstanceId`;
- monotonically changing frontend lifecycle epoch;
- activation/replacement/reset;
- immutable lifecycle capture;
- current/stale comparison and assertion.

Required semantic interface:

```ts
export interface ProjectLifecycleSnapshot {
  readonly projectInstanceId: string;
  readonly projectEpoch: number;
}

export function startProjectLifecycle(projectInstanceId: string): void;
export function clearProjectLifecycle(): void;
export function captureProjectLifecycle(): ProjectLifecycleSnapshot;
export function isProjectLifecycleCurrent(
  snapshot: ProjectLifecycleSnapshot,
): boolean;
export function assertProjectLifecycleCurrent(
  snapshot: ProjectLifecycleSnapshot,
): void;
```

Names may be adjusted to established terminology, but ownership and behavior are fixed.

The module may depend only on domain/core types and utilities. It must not import application, services, views, Tauri, publication coordinator, graph projection coordinator, or Zustand domain stores.

A missing active lifecycle and a stale snapshot must preserve the existing error code/message contract used by current callers.

## 2. Application orchestration

`projectPublicationCoordinator` remains responsible for:

- ordered publication application;
- direct/event echo deduplication;
- publication watermark tracking;
- gap detection and recovery;
- waiter cancellation and settlement.

It no longer owns the fundamental project identity state. Project activation/replacement/reset calls the Core lifecycle authority and then performs publication-specific reset work.

`projectCommandContext`, `graphProjectionCoordinator`, resource actions, project/session stores, and mutation event handlers capture/assert lifecycle directly through the Core authority. They must not route identity through the publication coordinator.

Operation IDs remain owned by command/application code. Backend project instance IDs remain backend-issued and are never synthesized.

## 3. Migration

Delete `src/features/application/projectIdentity.ts` after all consumers move to Core lifecycle authority. Do not leave a re-export, compatibility facade, or duplicate mutable state.

Migration must preserve:

- activation and replacement epoch behavior;
- stale direct result suppression;
- stale event rejection;
- old graph hydration suppression;
- publication waiter cancellation on replacement;
- pre-command capture/read/assert behavior;
- function/database/variable mutation correlation;
- test reset isolation.

No Resource Catalog DTO, watermark, database declaration, resource path, or Rust API changes belong to this task.

## 4. Architecture audit

Use the TypeScript compiler AST rather than regular-expression-only import scanning.

The audit scans production files under `src/services/**/*.{ts,tsx}` and resolves module specifiers relative to the importing file and the configured `@/` alias.

It must detect:

- static imports with bindings;
- side-effect imports;
- dynamic `import(...)` calls;
- CommonJS `require(...)` calls;
- TypeScript `import name = require(...)` declarations;
- alias paths;
- relative paths, including parent-directory traversal.

Any resolved target under `src/features/` or `src/views/` is forbidden.

The audit must include mutation fixtures proving each forbidden import form is detected and negative controls for allowed service/shared imports.

Add a focused lifecycle boundary audit proving:

- Core lifecycle authority imports no application/services/views modules;
- `projectPublicationCoordinator` and `graphProjectionCoordinator` both depend on Core authority rather than each other for identity;
- the deleted application identity facade cannot be reintroduced.

The audit must remain scoped; it must not impose a global ban on legitimate application-to-service dependencies.

## 5. Testing

Use RED-GREEN TDD.

Core lifecycle tests cover:

- no active lifecycle;
- first activation;
- replacement increments epoch and invalidates old snapshots;
- clear/reset invalidates old snapshots;
- immutable capture;
- stale assertion error compatibility;
- repeated activation semantics currently relied on by project loading.

Integration-focused tests cover:

- project replacement during revision authority read causes zero IPC/publication effects;
- old graph hydration cannot install into the replacement project;
- old direct/event publications are rejected;
- waiters are cancelled on replacement;
- project/session store reset behavior remains unchanged;
- command operation identity remains correlated with the captured backend project instance.

Architecture tests first demonstrate RED for side-effect, require, import-assignment, dynamic, alias, and relative service imports, then pass after the AST resolver is complete.

## 6. Verification

Run all lifecycle/identity consumers and architecture tests, followed by the Resource Catalog 24-file frontend aggregate established by the preceding plan.

Run the focused serial Rust filters from the preceding Resource Catalog/recovery plan to ensure no cross-boundary regression, then:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

An independent final reviewer must verify:

- one lifecycle owner;
- no application/core identity cycle;
- no core-to-application dependency for identity;
- no production service-to-feature/view imports through any supported syntax;
- unchanged publication, recovery, resource identity, and mutation behavior.

After fresh controller verification and clean review, update `.superpowers/sdd/2026-08-03-core-project-lifecycle-authority/progress.md` and `TODO.md` under `## node_architecture 进度`.
