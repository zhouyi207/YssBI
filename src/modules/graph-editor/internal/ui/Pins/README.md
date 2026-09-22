# Graph Pin UI

`GraphPinController` consumes one normalized Rust editor-projection `PinData` object. Component
callbacks and context-menu capabilities remain separate props and must never be copied into Canvas
interaction state.

Unconnected scalar input ports render `PinInput`. Editing submits `SetPinValue` to Rust's current
graph document and installs its authoritative response. Explicit Save persists that document.
Numeric and text inputs commit on blur, Boolean inputs commit immediately, Enter blurs, and Escape
restores the projected value.

Connection capacity, repeatable-port removal, type information, literal/default values, and port
diagnostics come from the Rust editor projection. The UI must not reconstruct those facts from an
old node-definition registry or infer required inputs from missing frontend metadata.

View queries the current result at an output address or the connected upstream outputs of an
input. It is available for outputs before execution and hidden for unconnected inputs; it never
starts execution. The Results application owns result queries and retained report leases.
