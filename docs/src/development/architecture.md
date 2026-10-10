# Architecture

Use this guide to find the owner of a change before modifying the desktop, application services or domain code. YssBI has a native GPUI Kit frontend and a Rust business layer in the same process. The frontend calls typed Application use cases rather than a separate UI command protocol.

## Dependency direction

```text
GPUI desktop
  -> Application services and typed use cases
      -> Project, Graph, Database, Execution and Harness
          -> domain contracts and infrastructure ports
  -> Logging
```

The desktop owns window composition, worker scheduling and presentation. Blocking business work runs off the UI interaction path. Domain and Application code do not depend on GPUI, desktop services or concrete business adapters.

Generic filesystem primitives are a foundation for Project and Application. `yss-filesystem` owns file access, transactions, change facts and watcher sessions, with no dependency on other repository crates. Project path conventions, document validation and business failures remain with Project and its adapters.

The `react/` tree is reference source. Its models and lifecycles do not define current native behavior.

## State ownership {#state-ownership}

| State or fact                                                       | Owner                             | Desktop responsibility                    |
| ------------------------------------------------------------------- | --------------------------------- | ----------------------------------------- |
| Project resources, revisions and committed state                    | Project                           | Read projections and invoke use cases     |
| Current graph document, undo/redo and saved-content identity        | Project                           | Render a one-way graph projection         |
| Resolved graph types, schemas, lineage, diagnostics and coercion    | `GraphSemanticSnapshot`           | Present Canvas, Details and Problems      |
| Dataset declarations, runtime and schema                            | Project and Database              | Query bounded pages                       |
| Execution identity, results and provenance                          | Execution and ResultStore         | Retain result references and query leases |
| Numerical algorithms                                                | SCI or an external compute plugin | Present returned reports and plots        |
| Plugin authorization, installation and task ledger                  | Plugin Manager                    | Present management views                  |
| Assistant conversations, turns and workflows                        | Harness and its persistence ports | Subscribe to projections and events       |
| Panel placement, ordering, selection, edge sizes and collapse state | Root GPUI `DockArea`              | Use the component's topology              |
| Viewport, dragging, selection and uncommitted input                 | Owning desktop view               | Keep transient interaction state          |

Do not create a second graph draft, undo stack or saved-content identity in the frontend. Replace graph projections from their owner in one direction. Other resource-specific drafts keep their documented owners.

Project instances, resource paths, graph sessions, nodes, ports, runs, results and panels have different identities. A late read or callback cannot install data into a different project, graph or view. Graph constants belong to their graph document, not to a separate global resource store.

## Editing, saving and execution

Manual graph edits update Project's current graph and its semantic projection. **Save** persists the current graph independently. Undo, redo and execution do not implicitly save the graph body. Assistant edit batches persist the complete current graph atomically and retain undo history, without requiring an editor panel.

Analysis graphs express data ports and dependencies. Control flow, effects and sequencing belong to Workflow owners. Execution captures the document and semantic identity, prepares a matching immutable plan internally, and returns run facts and result references. Plan caches are backend details, not a second frontend lifecycle.

Result views use Application leases and bounded page queries rather than recomputing statistics. Closing a result panel releases its lease; remounting it during a drag preserves its reading identity. An ordinary rerun does not replace the execution referenced by an already-open result.

See the [Graph application contract](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/src/graph/README.md) for the detailed edit, execution and result lifecycle.

## Node kernels and scientific computation

`yss-node-kernel` owns graph-independent invocation values, kernel contracts, the frozen registry and built-in adapters. It does not depend on Graph, Project, Application or desktop frameworks. Application composes one registry shared by readiness checks and execution. Graph addresses, authorization, plans, result storage and graph error locations remain with their respective owners.

The scientific call direction is:

```text
Node Kernel -> SCI Runtime -> SCI -> Linalg
```

Only Linalg depends on faer and owns matrix/vector wrappers. Runtime exposes stateless functions with neutral inputs and outputs. Only Node Kernel and the focused Graph Execution SCI benchmark consume SCI Runtime; the application and desktop do not construct a scientific backend. Julia plugin crates own their input and cancellation contracts independently of host SCI crates.

## Harness boundary {#harness-boundary}

Harness owns conversation and orchestration state, not a second copy of Project, Graph, Database or numerical results. Core stays provider-neutral through injected ports; framework, persistence and transport integrations remain adapters. The shared `yss-harness-contract` crate exposes platform-neutral values and Assistant read projections without GUI or runtime dependencies.

The internal Assistant calls the Application capability gateway directly. Routing in-process calls through loopback MCP would add a transport and competing lifecycle without adding authority. Desktop actions are not an automatic model-tool registry, and adapters must not bypass the existing business owners.

Statistical values come from executed capabilities and evidence, not from a model's invented numbers. Current capabilities and limitations are documented by [Harness Core](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-harness-core/README.md). MCP adapters and external/background write gates remain open work, not capabilities implied by this architecture.

## Signals, errors and privacy

Problems, Results, run state/failures and Logs are separate flows. Logs must not reconstruct or substitute for graph diagnostics, execution state or result data. `yss-logging` owns structured observations, collection, persistence and subscriptions; Rust producers use `tracing` and the native host owns localized delivery.

Application errors remain typed. Serialized command failures use `{ code, details, incidentId }`; business crates do not produce user-facing prose. The host localizes stable codes. User data, documents, model content and credentials do not belong in runtime logs.

Project calendar values and user-facing timestamps are timezone-free. Removing a timezone preserves the original calendar and wall-clock fields. Window closing, project switching and explicit saving use existing Application operations; failure preserves unsaved state.

## See also

- [Developing YssBI](../development.md)
- [Graphs & Results](../graphs-and-results.md)
- [Testing](./testing.md)
