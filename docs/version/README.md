# Version History

Version files are historical implementation records extracted from `TODO.md`.
They explain what changed during a release line; they are not the current
architecture contract.

## Canonical source order

When historical notes conflict with current behavior, use this order:

1. `AGENTS.md` — project rules for agents and contributors.
2. `docs/architecture/` — current architecture and design decisions.
3. `docs/development/` — current toolchain and verification workflow.
4. Module `README.md` files linked from `docs/README.md` — focused contracts.
5. `docs/version/` — historical implementation timeline only.

## Files

| File              | Version | Period                  | Meaning                                                                              |
| ----------------- | ------- | ----------------------- | ------------------------------------------------------------------------------------ |
| `v0_0.md`         | v0.0    | 2026-02-27 – 2026-05-20 | Initial statistics, graph, data, and UI foundation history                           |
| `v0_1.md`         | v0.1    | 2026-06-23 – 2026-06-30 | Data, editor, type-system, and UI convergence history                                |
| `v0_2.md`         | v0.2    | 2026-07-01 – 2026-08-24 | Resource lifecycle, graph architecture, execution, and workbench convergence history |
| `v0_3(待完成).md` | v0.3    | 2026-08-25 onward       | Open backlog carried into the next release line                                      |

## Status vocabulary

- `[x]` / `completed`: implemented and recorded as historical fact.
- `[ ]` / `planned`: proposal or open item at the time; verify against code and
  `TODO.md` before treating it as a current task.
- `deferred` / `暂缓`: intentionally postponed.
- `superseded` / `已 supersede`: replaced by a later design or implementation.
- `historical`: context only; current behavior is defined by the canonical
  documents above.

## Retrieval guidance

Index each section with its version, date heading, and feature heading. A
version entry may describe an intermediate implementation and may mention paths
that later moved; resolve paths against current canonical documents before
citing it as present-day behavior.
