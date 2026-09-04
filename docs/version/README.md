# Version History

> Status: Historical
> Scope: 已完成 release line 的历史实现记录索引
> Canonical owners: 本目录只拥有历史；当前行为由 code/tests/manifests 和 Current architecture 决定
> Update when: 一个 release line 完成并归档，或历史索引发生变化时

Version files record completed work, intermediate states, and decisions as they appeared during a release line. Paths and designs may have changed later; nothing in this directory proves current implementation behavior.

## Canonical source order

When history conflicts with maintained knowledge, use:

1. code, tests, and manifests for executable facts;
2. `docs/architecture/` for current architecture and stable contracts;
3. `.rules` for coding-agent behavior;
4. `docs/development/` for current workflow;
5. `docs/decisions/` for accepted rationale;
6. `docs/roadmap/` for unfinished plans;
7. `docs/version/` for historical context only.

## Archived release lines

| File                                                   | Version                | Period                  | Meaning                                                                 |
| ------------------------------------------------------ | ---------------------- | ----------------------- | ----------------------------------------------------------------------- |
| [v0_0.md](v0_0.md)                                     | v0.0                   | 2026-02-27 – 2026-05-20 | Initial statistics, graph, data, and UI foundation history              |
| [v0_1.md](v0_1.md)                                     | v0.1                   | 2026-06-23 – 2026-06-30 | Data, editor, type-system, and UI convergence history                   |
| [v0_2.md](v0_2.md)                                     | v0.2                   | 2026-07-01 – 2026-08-24 | Resource lifecycle, graph, execution, and workbench convergence history |
| [legacy-todo-2026-09-04.md](legacy-todo-2026-09-04.md) | pre-migration snapshot | through 2026-09-04      | Former mixed TODO/backlog/change-log content, preserved read-only       |

Open v0.3/v1.0 items live in [`docs/roadmap/`](../roadmap/), not here. A roadmap moves into this directory only after the release line is completed and its entries have been converted into historical facts.

## Retrieval guidance

- `[x]` describes something recorded as completed at that historical point.
- `[ ]`, planned, deferred, or superseded text remains historical context; check current roadmap/code before treating it as open work.
- Resolve old paths and names against the current [documentation index](../README.md).
- Do not copy historical architecture descriptions back into Current documents without verifying the implementation.
