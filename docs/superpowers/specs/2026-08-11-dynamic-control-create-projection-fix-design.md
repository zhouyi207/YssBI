# Dynamic Control Create Projection Fix Design

## Problem

Creating a node from an unconnected dynamic `yssbi.control.merge` input commits the
Rust mutation and advances the graph revision, but the frontend rejects the returned
projection replacement. An internal lowering failure currently serializes its node
field as `node_id`; the strict frontend wire parser requires `nodeId`. The frontend
therefore remains on the previous projection revision, and the next compatible-catalog
request is rejected as stale.

The merge node also materializes its required `enter` inputs with stable document
orders (`00000`, `00001`) but projects them through UUID-keyed map order. The input
chosen authoritatively as the first candidate can consequently appear as the second
input in the UI.

## Design

### Compilation outcome wire

Rust remains the authoritative wire owner. `CompilationOutcomeDto` must serialize all
variant fields in camelCase, including `internalFailure.nodeId`. The frontend parser
remains strict and continues rejecting snake_case or additional compatibility shapes.
No migration shim is added in this 0.x project.

### Dynamic port order

Resolved interfaces must preserve protocol/document order rather than UUID address
order. Declared ports follow protocol declaration order. Dynamic instances of the same
template follow their persisted `OrderKey`, with the stable address used only as a
deterministic tie-breaker. The projection and UI consume this authoritative order
without re-sorting dynamic pins.

This makes the instance with order `00000` both the backend's first connection
candidate and the first visible `enter` input.

### Failure and revision behavior

Internal compilation failures remain valid committed projections with blocking state;
they must not cause transport/parser failure. Once the corrected projection is parsed,
the frontend installs revision 2 normally, so subsequent compatible-catalog requests
use revision 2. No parser-failure recovery or forced projection reload is introduced.

## Tests

1. A focused Rust wire test reproduces dynamic merge input create-and-connect and
   asserts that an internal failure serializes `nodeId` and not `node_id`.
2. A focused dynamic-interface/projection test asserts that user-created ports with
   orders `00000` and `00001` are emitted in that order regardless of UUID ordering.
3. Existing TypeScript mutation parser and Rust-generated contract tests remain green.
4. Validation uses Cargo `--jobs 2 --test-threads=2`, Vitest `--maxWorkers=2`, and
   `git diff --check`.

## Scope

Only compilation-outcome serialization, resolved dynamic-port ordering, and focused
regression coverage are changed. Control-flow lowering semantics and mutation revision
semantics are unchanged.
