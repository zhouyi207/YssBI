# Project Publication Recovery State Machine

## Status

Approved focused design for resolving the remaining frontend publication ordering, recovery, and move-application failures that block the project filesystem transaction plan.

## Problem

Direct mutation responses and `ResourceMutationCommitted` events can arrive out of order or duplicate each other. The current promise queue serializes by arrival rather than publication revision, treats some failed hydrations as successful, and can mutate path-owned stores before later asynchronous work fails. Gap recovery can race a newly arrived missing revision or leave callers pending when recovery itself fails.

These failures make the frontend publication watermark, history status, resource paths, and graph projections diverge even when Rust authority is correct.

## Goals

- Give one project-scoped coordinator exclusive ownership of resource publication application.
- Apply publications in backend `publicationRevision` order rather than arrival order.
- Make every fallible asynchronous preparation complete before any path-owned store mutation.
- Resolve revision gaps through a complete authoritative project resynchronization.
- Settle every caller deterministically; no permanently pending promises.
- Prevent old async work from mutating a replacement project.
- Never automatically replay a mutation after recovery because Rust has already decided whether it committed.

## State

```ts
interface ProjectPublicationState {
  projectInstanceId: string
  epoch: number
  appliedRevision: number
  phase: 'idle' | 'applying' | 'recovering'
  pendingByRevision: Map<number, PendingPublication>
}
```

`PendingPublication` owns one validated result fingerprint and a shared promise/deferred list for all matching direct/event deliveries.

A second payload with the same revision but a different fingerprint is a protocol error.

## Ownership boundary

`ProjectPublicationCoordinator` is the only component allowed to:

- accept direct resource mutation results;
- accept `ResourceMutationCommitted` events;
- apply resource moves;
- update resource publication watermark;
- publish backend history status from resource results;
- trigger publication-gap recovery;
- settle duplicate direct/event waiters.

Event handlers, history coordinators, function mutation coordinators, and rename actions delegate to it and do not update these stores independently.

## Ordered application

A normal publication is eligible only when:

```text
publicationRevision == appliedRevision + 1
```

The coordinator performs:

1. project instance and epoch validation;
2. complete wire, delta, move, projection-membership, and fingerprint validation;
3. all fallible asynchronous preparation;
4. epoch revalidation;
5. synchronous store commit with no intervening `await`;
6. history status and watermark publication last.

Asynchronous preparation includes destination graph preload, affected graph hydration, and required function metadata refresh. A `false` result is a failure, not success.

## Move application

Resource moves use two phases.

### Prepare

- preload the destination graph projection;
- hydrate unchanged caller graph paths required by incomplete status;
- validate destination metadata, including authoritative destination name;
- build a pure move application plan for resource metadata, tabs, graph sessions, document state, projection ownership, caller references, and variable scopes.

No path-owned store is changed during prepare.

### Commit

After one final project identity and epoch check, apply the prepared plan synchronously without `await`. The commit functions are prevalidated and non-throwing. History status and publication watermark update only after the move plan commits.

If prepare fails, no move/path state has changed and the coordinator enters authoritative recovery.

## Reverse arrival and gaps

If publication `N+1` arrives while `appliedRevision + 1 == N`, the coordinator stores it but does not install it. It starts authoritative recovery immediately rather than waiting indefinitely.

During recovery, later publications are queued but not applied.

Recovery rechecks `appliedRevision` before every decision. If the missing revision arrives and advances the watermark while recovery is awaiting I/O, recovery observes that advancement and must not reject publications that are now contiguous or already applied.

## Authoritative recovery

Recovery captures the current `projectInstanceId` and epoch, then obtains one canonical recovery snapshot containing:

- project index and current project instance;
- backend resource publication revision;
- function signatures and their revisions;
- backend history status;
- authoritative resource paths and display names.

It hydrates:

- every graph loaded when recovery began;
- every graph declared by queued publications as expected or invalidated;
- destination graph paths declared by queued moves.

Before changing stores it revalidates project identity and epoch. It then performs one synchronous reconciliation of resources, graph projections, function metadata, sessions/tabs, and history, and sets `appliedRevision` to the backend snapshot revision.

Queued publications at or below that revision settle as:

```ts
{ status: 'recovered' }
```

They are not replayed. Higher contiguous publications resume normal draining.

## Recovery failure

A failed index request, metadata request, or graph hydration:

- rejects every queued waiter owned by that recovery attempt;
- clears the recovery and in-flight entries;
- returns phase to `idle`;
- leaves watermark and history unchanged;
- marks the project projection stale;
- permits a later operation to start a fresh recovery attempt.

There is no automatic infinite retry and no unresolved promise.

## Lifecycle changes

Project replacement increments the frontend project epoch and synchronously:

- rejects all old pending publication waiters with `stale_project_lifecycle`;
- clears pending/in-flight/recovery state;
- resets the publication baseline from the new project index.

Every asynchronous step checks the captured identity and epoch after `await` and before any side effect.

## Backend requirements

`ProjectIndex` exposes the current backend resource publication revision so recovery can establish a canonical baseline.

`ResourceMoveDto` requires authoritative destination display identity:

```ts
interface ResourceMoveDto {
  kind: 'graph'
  from: string
  to: string
  name: string
}
```

Rename, undo, and redo populate `name` for the destination direction.

## Error model

Coordinator outcomes distinguish:

- `applied`;
- `duplicate`;
- `recovered`;
- `stale_project_lifecycle`;
- `publication_protocol_error`;
- `publication_recovery_failed`.

Gap detection is not reported as a mutation conflict. The backend operation may already be committed; recovery reconciles authority rather than replaying it.

## Tests

Focused tests must prove:

- reverse arrival `N+1` then `N` cannot install out of order;
- recovery racing a newly arrived missing revision does not reject already contiguous work;
- failed recovery rejects all queued waiters and leaves no pending entries;
- a later submission can retry recovery;
- move preload or caller hydrate returning `false` produces no path-owned store mutation;
- move retry preserves metadata and document flags;
- direct/event duplicates share one promise and one commit;
- project replacement rejects old queued and recovering work;
- rename, undo, and redo install authoritative destination names;
- recovery establishes watermark, function metadata, graph projections, resources, and history from one backend snapshot.

Only explicit Vitest files and focused Rust serde/index tests are allowed. The complete Rust suite remains reserved for the final filesystem transaction checkpoint.

## Integration with the Filesystem Transaction Plan

This design is a focused prerequisite repair for Task 2 of `2026-07-27-project-filesystem-transaction.md`. After it passes review:

1. repair the mismatched Task 2 report artifacts;
2. mark the previous five-round Task 2 loop superseded by this approved recovery sub-plan;
3. re-review Task 2 as a whole;
4. resume Task 3 only after the publication coordinator is clean.

No compatibility event, alternate result applier, or dual watermark owner remains.
