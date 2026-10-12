# Architecture

Use this guide to find the owner of a change before modifying the desktop, application services or domain code. YssBI has a native GPUI Kit frontend and a Rust business layer in the same process. The frontend calls typed Application use cases rather than a separate UI command protocol.

## Dependency direction

The binary composes Workbench; Workbench depends on feature UI crates, never the reverse. Features call typed Application use cases and consume domain projections. Shared desktop scheduling and event delivery live below the views, without GPUI.

| Crate                  | Responsibility                                                                                                                                         |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `yss-desktop-gpui`     | Arguments, the process Tokio runtime, GPUI initialization and final window composition                                                                 |
| `yss-workbench`        | Startup, project navigation, root DockArea, panel registration, layout restoration and cross-feature routing                                           |
| `yss-ui`               | Shared product presentation, localization, Markdown/math, preferences, keymap, window chrome and layout storage                                        |
| `yss-desktop-services` | `ApplicationTasks` over the existing Application state/runtime handle, the shared `DesktopEvents` bus and readonly resource catalog                    |
| `yss-graph-ui`         | GraphCanvas, readonly OpenedGraph, retained GraphDetails, graph forms, node catalog rows, diagnostic/run-failure presentation and graph result catalog |
| `yss-assistant-ui`     | Retained AssistantController, conversation panels, directory/cache ownership and Assistant UI workflows                                                |
| `yss-data-ui`          | Dataset browser/editor and import forms                                                                                                                |
| `yss-results-ui`       | Leased result readers, reports, chart editor and plots; Workbench coordinates independent result windows                                               |
| `yss-settings-ui`      | Retained SettingsPanel, model/knowledge settings and preference editing                                                                                |
| `yss-resource-ui`      | Markdown documents, mind maps and their captured-version save requests                                                                                 |
| `yss-plugin-ui`        | Plugin management and native plugin views over the canonical PluginManager                                                                             |

All GUI crates inherit the same workspace `gpui-kit` dependency. They do not introduce Zed's UI, theme, workspace or GPUI dependency chain. Domain, Application and `yss-desktop-services` remain GPUI-free.

`yss-ui` has no direct or transitive dependency on Application, Project or graph execution. Its internal dependencies are limited to canonical hashing, data contracts, filesystem, localization and settings. Graph-specific catalog rows and feedback belong to `yss-graph-ui`. `ActivityText::resolve_with` accepts the caller's translator for keys and preserves literal text; shared localization does not import Application types.

The readonly `ResourceCatalog` lives in `yss-desktop-services::resources`. Workbench builds it from the accepted ProjectIndex and shares the same `Arc` with AssistantController and cached conversations. It preserves project identity, resource/signature revisions and search indexes without introducing a second query, mutable model or GPUI dependency.

`NativeServices` is private to Workbench's composition layer. Features receive narrow handles for their actual capabilities, not that aggregate. `ApplicationTasks` reuses the process Tokio handle for blocking use cases; it does not start a second runtime or own models, plugins, preferences and logging. Typed events and callbacks carry cross-feature requests, including import submission and resource/result navigation.

`yss-application::runtime` remains the concrete adapter composition root. There is no separate bootstrap crate without a second host or other demonstrated isolation need. Project's operation admission/replay ledger is now its private `project_operation` module; Application's display-name allocator is its private `display_naming` module. Shared identity, layout, schema and scientific contracts remain separate crates.

Generic filesystem primitives are a foundation for Project and Application. `yss-filesystem` owns file access, transactions, change facts and watcher sessions, with no dependency on other repository crates. Project path conventions, document validation and business failures remain with Project and its adapters.

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

Compatible-node queries pass graph identity, editing version and source port, not a caller-owned graph document. Application reads the shared resident document and checks its version before and after computation. Worker tasks and concurrent edits still require these checks even without IPC.

Project instances, resource paths, graph sessions, nodes, ports, runs, results and panels have different identities. A late read or callback cannot install data into a different project, graph or view. Graph constants belong to their graph document, not to a separate global resource store.

## Retained desktop state

Workbench retains feature entities and coordinates their placement; it does not own their internal editing workflows. `GraphDetails` retains graph-specific forms. Settings drafts remain in the same `SettingsPanel` when its independent window closes. Import forms emit `ImportSubmission`; they do not retain or depend on a concrete Workbench.

`AssistantController` owns the strong conversation cache, directory projection, refresh generations and conversation mutations. Each cached `ConversationPanel` owns its transient input. Hiding the conversation column removes panels from DockArea but preserves the cache and subscriptions. Beginning a project transition invalidates pending delivery; only an accepted project identity change resets the old cache. A failed activation does not discard its drafts.

Workbench's controller subscription and observation survive replacement of project-bound subscriptions. Layout factories hold a weak controller handle rather than a second strong cache. DockArea remains the only topology owner; the controller never keeps an alternate selected panel or group tree.

Read generations, project identities and resource versions still reject stale delivery. Dropping a view task does not imply that an already-started blocking write was cancelled; save receipts continue to be accepted through the original owners.

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
