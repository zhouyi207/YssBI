# Graph Pin UI

`GraphPinController` consumes one normalized Rust editor-projection `PinData` object. Component
callbacks and context-menu capabilities remain separate props and must never be copied into Canvas
interaction state.

Variable inputs display the Rust-projected titles (`Y`, `X₁`, `X₂`, …).
Input tooltips read connected source labels through the existing Pin adjacency index;
no full-graph scan or frontend numbering is needed. Canvas and Details use the same projected names.

Each Pin controller is a memoized render boundary using the store's shared immutable Pin reference.
Node layouts pass the stable `renderPinHandle` function instead of creating a handle element for every
row on each node update. The controller creates and caches its own handle element by Pin and function
identity, so title, parameter or sibling-port updates reuse unchanged controllers and handles.
Pin diagnostics, results, theme, locale and local menu/input state retain their existing subscriptions;
the handle's own connection-feedback subscription continues independently. Changing the Pin, graph,
menu actions or renderer updates the affected component through ordinary React props.

Unconnected scalar input ports render `PinInput`. Editing submits `SetPinValue` to Rust's current
graph document and installs its authoritative response. Explicit Save persists that document.
Numeric and text inputs commit on blur, Boolean inputs commit immediately, Enter blurs, and Escape
restores the projected value.
The controller caches the input element by its visibility, graph/port identity, type and projected
value. Result, diagnostic, theme and menu decoration updates reuse that element; its own focus and
editing state continue to update independently. Connection or type changes still remove unsupported
inputs, and projection changes still deliver new values through the same editing hook.

Connection capacity, repeatable-port removal, type information, literal/default values, and port
diagnostics come from the Rust editor projection. The UI must not reconstruct those facts from an
old node-definition registry or infer required inputs from missing frontend metadata.

Primary port diagnostics are read by port ID from the existing Graph projection
store's sparse `primaryPortDiagnostics` index. The store shares the original diagnostic and
updates the index atomically with the node; Pin subscribers do not scan diagnostic lists.
The first blocking diagnostic takes precedence, otherwise the first diagnostic is shown.
Diagnostic text is formatted once per diagnostic reference and resolved language, then reused by
the tooltip and diagnostic marker instead of repeating formatting on execution appearance updates.

View queries the current result at an output address or the connected upstream outputs of an
input. It is available for outputs before execution and hidden for unconnected inputs; it never
starts execution. The Results application owns result queries and retained report leases.
Only an open context menu subscribes to View availability, as a boolean read from the existing
connection adjacency index. Closed menus add no connection subscription or result-reference
allocation. Clicking View resolves targets from the current graph snapshot in the application;
the UI does not retain a connection array captured during rendering.
