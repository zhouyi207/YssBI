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
| `yss-resource-ui`      | Markdown documents, mind maps and the authored-file session shared with the chart editor                                                               |
| `yss-plugin-ui`        | Plugin management and native plugin views over the canonical PluginManager                                                                             |

All GUI crates inherit the same workspace `gpui-kit` dependency. They do not introduce Zed's UI, theme, workspace or GPUI dependency chain. Domain, Application and `yss-desktop-services` remain GPUI-free.

`yss-ui` has no direct or transitive dependency on Application, Project or graph execution. Its internal dependencies are limited to canonical hashing, data contracts, filesystem, localization and settings. Graph-specific catalog rows and feedback belong to `yss-graph-ui`. `ActivityText::resolve_with` accepts the caller's translator for keys and preserves literal text; shared localization does not import Application types.

The readonly `ResourceCatalog` lives in `yss-desktop-services::resources`. Workbench builds it from the accepted ProjectIndex and shares the same `Arc` with AssistantController and cached conversations. It preserves project identity, resource/signature revisions and search indexes without introducing a second query, mutable model or GPUI dependency.

`query_project_index` returns the raw `ProjectIndex`; only `query_project_presentation` builds localized activity panels. Both use one captured session/publication and reject stale delivery after projection. Consuming read receipts transfers owned graph projections and database schema columns instead of deep-cloning them.

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
| Assistant conversations and turns                                   | Harness and its persistence ports | Subscribe to projections and events       |
| Panel placement, ordering, selection, edge sizes and collapse state | Root GPUI `DockArea`              | Use the component's topology              |
| Viewport, dragging, selection and uncommitted input                 | Owning desktop view               | Keep transient interaction state          |

Do not create a second graph draft, undo stack or saved-content identity in the frontend. Replace graph projections from their owner in one direction. Other resource-specific drafts keep their documented owners.

Compatible-node queries pass graph identity, editing version and source port, not a caller-owned graph document. Application reads the shared resident document and checks its version before and after computation. Superseded read-only queries cooperatively stop between preparation stages and candidate scans; cancellation returns no partial candidate list and never cancels a committed edit.

Project instances, resource paths, graph sessions, nodes, ports, runs, results and panels have different identities. A late read or callback cannot install data into a different project, graph or view. Graph constants belong to their graph document, not to a separate global resource store.

## Retained desktop state

Workbench retains feature entities and coordinates their placement; it does not own their internal editing workflows. Each `GraphCanvas` retains its nodes' parameter forms and inline input drafts; `GraphDetails` mounts the selected form rather than replacing its owner. Settings drafts remain in the same `SettingsPanel` when its independent window closes. Import forms emit `ImportSubmission`; they do not retain or depend on a concrete Workbench.

`AssistantController` owns the strong conversation cache, directory projection, refresh generations and conversation mutations. Each cached `ConversationPanel` owns its transient input. Hiding the conversation column removes panels from DockArea but preserves the cache and subscriptions. Beginning a project transition invalidates pending delivery; only an accepted project identity change resets the old cache. A failed activation does not discard its drafts.

Workbench's controller subscription and observation survive replacement of project-bound subscriptions. Layout factories hold a weak controller handle rather than a second strong cache. DockArea remains the only topology owner. Workbench retains one active editor identity for command targeting, validates it against live Dock placement, and does not derive it from the current Details inspection. This identity is not another selected-panel or group tree.

Menus project command availability rather than owning another editor state. Keep the originating input focus and availability together while AppMenuBar owns focus. Text edit commands restore that input after menu dismissal; unavailable text actions never fall through to graph commands. Menu actions retain the same identities as keyboard bindings so shortcut hints and customization stay consistent.

High-frequency state does not flow through a single Workbench notification. `GraphStatistics` owns the viewport readout; the command observer reacts to busy, dirty, undo/redo and editing-command availability, not viewport coordinates. `GraphProperties` relays editing-state/version or projection-identity changes, not every pan or zoom. Problems, Output and Results read one project-wide `ProjectOverview`, which coalesces graph activity into background Application reads and notifies only after a read completes; switching editors does not rebuild them.

Graph edit notifications carry the originating operation identity. A canvas acknowledges its own installed command receipts without scheduling a duplicate read, regardless of event/receipt ordering. Unrelated edits, dependency changes and result changes still invalidate it, including changes at the same document revision. A failed command acknowledges only its successfully committed prefix.

Unbound-port diagnostics are static: a repeating animation schedules new frames and invalidates its owning canvas even when the user is idle. Run activity therefore animates in its own `ActivityOverlay` view, so its frames repaint the pulses without rebuilding the canvas. The grid coarsens at low zoom while retaining world alignment, fading the finer level in rather than switching levels at once. Keep caching at the existing DockSkin panel and toolbar boundaries, not around the entire Workbench or DockArea; nested cache misses can force child rebuilds, and conversation rendering has frame-local prepaint transfers.

The existing connection layer also retains node layouts, port indexes and anchors. Pure position changes reuse presentation only while language, results and non-position projection content still match. Nodes, ports, connections and the grid are painted by one canvas element rather than mounted as elements, so a frame's layout work does not grow with the graph; only inline controls, tooltip anchors and the located-port focus target are elements. Viewport size changes schedule a settling frame so newly visible inline controls are mounted without another pointer event. Inline controls reconcile on projection, visible-node set or language changes; clean offscreen retention is bounded, while focused, dirty, pending and invalid controls remain available. Canvas labels are shaped once per content, font family and UI scale; zooming scales their glyph geometry instead of shaping them again.

The desktop uses the published GPUI Kit 0.7.3 facade with its optional experimental `gpui-fast` backend; Cargo.lock resolves GPUI Fast 0.1.8. Cached accessibility semantics and action listeners are replayed by the upstream backend alongside visual content. Dock caching remains enabled, and application code uses only the backend types selected by the Kit facade.

The repository carries neither a complete third-party source snapshot nor a local Cargo patch. Prefer registry releases with reproducible versions and a lockfile. A necessary unreleased fix belongs in an authorized external dependency repository pinned to an immutable commit, with its upstream issue, patch scope and removal condition documented. Machine-specific paths, package-cache edits and build-time source rewriting are not alternatives to this ownership boundary.

Assistant history reduction and acknowledgement analysis run on a blocking worker, with session/generation and live-event ordering guards on delivery. Composer predicates borrow the input Rope. Collapsed cell previews and column details do not synchronize or build hidden bodies; reopening uses the latest selection.

`LayoutStore` binds the process runtime handle once and updates existing viewport entries in place. Its 250 ms debounce rejects superseded stamps before dispatching a blocking write and revalidates at persistence. Window close flushes the latest window and viewport state rather than waiting for a timer.

Read generations, project identities and resource versions still reject stale delivery. Dropping a view task does not imply that an already-started blocking write was cancelled; save receipts continue to be accepted through the original owners.

## Editing, saving and execution

Manual graph edits update Project's current graph and its semantic projection. **Save** persists the current graph independently and preserves undo/redo history; replacing it with a different complete body discards history that no longer applies. Undo, redo and execution do not implicitly save the graph body. Assistant edit batches persist the complete current graph atomically and retain undo history, without requiring an editor panel.

Save and run capture pending parameter and port input through the canvas owner. Invalid text or a changed field base blocks the operation without losing the draft. A native mutation batch reuses one private document candidate, builds one final projection and records one undo entry. Persisting or starting execution follows that accepted edit; failures keep the committed edit and remaining drafts visible rather than pretending the whole workflow was rolled back.

Node dragging retains shared original positions plus a displacement. Pointer moves do not rebuild the selected-position map; release submits one version-checked move, while cancellation and no-op gestures submit none. Connection geometry survives projection refreshes by connection identity, with endpoint and port-order changes invalidating the relevant cache entries.

Documents, mind maps and charts are authored files with one editing session (`yss-resource-ui::file_session`): the session owns the file snapshot, save and refresh state, conflicts with a newer version and removal, while each editor keeps only its draft. Graphs keep their own editing model, but Workbench treats every open editor alike where the concern is shared: one route (`CommittedResourceMutation::routes`) delivers each committed receipt — moves, deletions and changes, whether from the GUI, the Assistant or an external writer — to the open graph or file editor, one flow renames, duplicates and deletes graphs and files, and unsaved/busy checks, saving, edit commands, index delivery and resynchronisation dispatch through one editor handle. Captured file, database and chart saves use their `ApplicationTasks` owner; Workbench save requests use its private `NativeServices`. They do not accept a second Application instance or runtime handle. Operation identities, session/version checks and edited snapshots on persistence failure remain part of the save contract.

Analysis graphs express data ports and dependencies. Control flow, effects and sequencing are not graph nodes. Execution captures the document and semantic identity, prepares a matching immutable plan internally, and returns run facts and result references. Plan caches are backend details, not a second frontend lifecycle.

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
